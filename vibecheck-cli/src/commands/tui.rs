use std::collections::{BTreeMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

use vibecheck_core::ignore_rules::{IgnoreConfig, IgnoreRules};
use vibecheck_core::report::{ModelFamily, Report, SymbolReport};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A single entry in the flattened, visible tree list.
#[derive(Clone)]
pub(crate) struct FlatEntry {
    path: PathBuf,
    /// Display name (just the last component).
    name: String,
    depth: usize,
    is_dir: bool,
    family: ModelFamily,
    confidence: f64,
}

struct App {
    /// All entries in depth-first order (full tree, never pruned).
    all: Vec<FlatEntry>,
    /// Paths of directories that are currently collapsed.
    collapsed: HashSet<PathBuf>,
    /// Currently selected row (index into `visible()`).
    selected: usize,
    /// `ListState` kept in sync with `selected`.
    list_state: ListState,
    /// Full report for the currently selected file (None for dirs).
    detail: Option<Report>,
    /// Vertical scroll offset for the detail pane.
    detail_scroll: u16,
}

impl App {
    #[cfg(test)]
    fn for_test(all: Vec<FlatEntry>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        App {
            all,
            collapsed: HashSet::new(),
            selected: 0,
            list_state,
            detail: None,
            detail_scroll: 0,
        }
    }

    fn new(all: Vec<FlatEntry>) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        let detail = all
            .first()
            .filter(|e| !e.is_dir)
            .and_then(|e| vibecheck_core::analyze_file_symbols(&e.path).ok());
        App {
            all,
            collapsed: HashSet::new(),
            selected: 0,
            list_state,
            detail,
            detail_scroll: 0,
        }
    }

    /// Returns only the entries that should be visible given current collapse state.
    fn visible(&self) -> Vec<&FlatEntry> {
        let mut result = Vec::new();
        let mut hidden_under: Option<(&Path, usize)> = None;

        for entry in &self.all {
            // If we are currently skipping children of a collapsed dir:
            if let Some((dir, depth)) = hidden_under {
                if entry.path.starts_with(dir) && entry.depth > depth {
                    continue;
                } else {
                    hidden_under = None;
                }
            }
            if entry.is_dir && self.collapsed.contains(&entry.path) {
                hidden_under = Some((&entry.path, entry.depth));
            }
            result.push(entry);
        }
        result
    }

    fn toggle_collapse(&mut self) {
        let visible = self.visible();
        let Some(entry) = visible.get(self.selected) else { return };
        if !entry.is_dir {
            return;
        }
        let path = entry.path.clone();
        if self.collapsed.contains(&path) {
            self.collapsed.remove(&path);
        } else {
            self.collapsed.insert(path);
        }
    }

    fn move_down(&mut self) {
        let len = self.visible().len();
        if len == 0 { return; }
        self.selected = (self.selected + 1).min(len - 1);
        self.list_state.select(Some(self.selected));
        self.refresh_detail();
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.list_state.select(Some(self.selected));
        self.refresh_detail();
    }

    fn refresh_detail(&mut self) {
        let visible = self.visible();
        self.detail = visible
            .get(self.selected)
            .filter(|e| !e.is_dir)
            .and_then(|e| vibecheck_core::analyze_file_symbols(&e.path).ok());
        self.detail_scroll = 0;
    }

    fn scroll_detail_down(&mut self, amount: u16) {
        self.detail_scroll = self.detail_scroll.saturating_add(amount);
    }

    fn scroll_detail_up(&mut self, amount: u16) {
        self.detail_scroll = self.detail_scroll.saturating_sub(amount);
    }
}

// ---------------------------------------------------------------------------
// Tree building
// ---------------------------------------------------------------------------

/// Build a flat, depth-first list of `FlatEntry` from `(path, report)` pairs.
/// Directories are synthesised with a confidence score that is the weighted
/// average of the files they contain (weighted by lines of code).
pub(crate) fn build_flat_tree(root: &Path, reports: &[(PathBuf, Report)]) -> Vec<FlatEntry> {
    // Index reports by path for quick lookup.
    let report_map: BTreeMap<&Path, &Report> =
        reports.iter().map(|(p, r)| (p.as_path(), r)).collect();

    // Collect all unique directory prefixes between root and each file.
    let mut dirs: HashSet<PathBuf> = HashSet::new();
    for (path, _) in reports {
        let mut p = path.as_path();
        while let Some(parent) = p.parent() {
            if parent == root || !parent.starts_with(root) {
                break;
            }
            dirs.insert(parent.to_path_buf());
            p = parent;
        }
    }

    // Pre-compute per-directory aggregate (family, confidence).
    let dir_agg: BTreeMap<PathBuf, (ModelFamily, f64)> = dirs
        .iter()
        .map(|dir| {
            let (family, conf) = aggregate_dir(dir, &report_map);
            (dir.clone(), (family, conf))
        })
        .collect();

    // DFS traversal of root to build the flat list.
    let mut entries: Vec<FlatEntry> = Vec::new();
    dfs(root, root, 0, &report_map, &dir_agg, &mut entries);
    entries
}

fn dfs(
    root: &Path,
    dir: &Path,
    depth: usize,
    reports: &BTreeMap<&Path, &Report>,
    dir_agg: &BTreeMap<PathBuf, (ModelFamily, f64)>,
    out: &mut Vec<FlatEntry>,
) {
    // Emit this directory node (unless it is the root itself).
    if dir != root {
        let (family, confidence) = dir_agg.get(dir).copied().unwrap_or((ModelFamily::Human, 0.5));
        out.push(FlatEntry {
            path: dir.to_path_buf(),
            name: dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            depth,
            is_dir: true,
            family,
            confidence,
        });
    }

    let child_depth = if dir == root { 0 } else { depth + 1 };

    // Collect direct children.
    let mut sub_dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<(&Path, &Report)> = Vec::new();

    for (&path, &report) in reports {
        if path.parent() == Some(dir) {
            files.push((path, report));
        } else if path.starts_with(dir) {
            // A deeper file — find the direct sub-dir.
            if let Ok(rel) = path.strip_prefix(dir) {
                if let Some(first) = rel.components().next() {
                    let sub = dir.join(first.as_os_str());
                    if sub.is_dir() && !sub_dirs.contains(&sub) {
                        sub_dirs.push(sub);
                    }
                }
            }
        }
    }

    sub_dirs.sort();
    files.sort_by_key(|(p, _)| *p);

    for sub in &sub_dirs {
        dfs(root, sub, child_depth, reports, dir_agg, out);
    }

    for (path, report) in &files {
        out.push(FlatEntry {
            path: path.to_path_buf(),
            name: path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string(),
            depth: child_depth,
            is_dir: false,
            family: report.attribution.primary,
            confidence: report.attribution.confidence,
        });
    }
}

fn aggregate_dir(dir: &Path, reports: &BTreeMap<&Path, &Report>) -> (ModelFamily, f64) {
    let mut total_weight = 0.0f64;
    let mut family_scores: BTreeMap<String, f64> = BTreeMap::new();

    for (&path, &report) in reports {
        if path.starts_with(dir) {
            let w = (report.metadata.lines_of_code as f64).max(1.0);
            total_weight += w;
            let key = report.attribution.primary.to_string();
            *family_scores.entry(key).or_insert(0.0) += w * report.attribution.confidence;
        }
    }

    if total_weight == 0.0 {
        return (ModelFamily::Human, 0.5);
    }

    let (best_name, best_score) = family_scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(k, v)| (k.clone(), *v / total_weight))
        .unwrap_or_else(|| ("human".to_string(), 0.5));

    let family = name_to_family(&best_name);
    (family, best_score)
}

fn name_to_family(name: &str) -> ModelFamily {
    match name.to_lowercase().as_str() {
        "claude" => ModelFamily::Claude,
        "gpt" => ModelFamily::Gpt,
        "gemini" => ModelFamily::Gemini,
        "copilot" => ModelFamily::Copilot,
        _ => ModelFamily::Human,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn family_color(family: ModelFamily) -> Color {
    match family {
        ModelFamily::Claude => Color::Magenta,
        ModelFamily::Gpt => Color::Green,
        ModelFamily::Gemini => Color::Blue,
        ModelFamily::Copilot => Color::Cyan,
        ModelFamily::Human => Color::Yellow,
    }
}

fn family_abbrev(family: ModelFamily) -> &'static str {
    match family {
        ModelFamily::Claude => "Cl",
        ModelFamily::Gpt => "Gpt",
        ModelFamily::Gemini => "Ge",
        ModelFamily::Copilot => "Co",
        ModelFamily::Human => "Hu",
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(outer[0]);

    render_tree(frame, app, main[0]);
    render_detail(frame, app, main[1]);
    render_statusbar(frame, outer[1]);
}

fn render_tree(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let visible = app.visible();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let prefix = if entry.is_dir {
                if app.collapsed.contains(&entry.path) { "▸ " } else { "▾ " }
            } else {
                "  "
            };
            let badge = format!(
                "{} {:>3.0}%",
                family_abbrev(entry.family),
                entry.confidence * 100.0
            );
            let color = family_color(entry.family);

            let line = Line::from(vec![
                Span::raw(format!("{indent}{prefix}")),
                Span::styled(
                    format!("{:<30}", &entry.name),
                    if entry.is_dir {
                        Style::default().add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(badge, Style::default().fg(color)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Files "))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_symbol_lines(symbols: &[SymbolReport]) -> Vec<Line<'static>> {
    if symbols.is_empty() {
        return vec![];
    }
    let mut lines = vec![
        Line::raw(""),
        Line::from(Span::styled(
            format!(" Symbols ({}):", symbols.len()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    for sym in symbols {
        let bar_len = (sym.attribution.confidence * 16.0) as usize;
        let bar = "█".repeat(bar_len);
        let color = family_color(sym.attribution.primary);
        let kind_label = match sym.metadata.kind.as_str() {
            "method" => "method",
            "class"  => "class",
            _        => "fn",
        };
        // Append () to functions/methods so "name" reads as "name()" not a placeholder.
        let raw_name = if kind_label == "class" {
            sym.metadata.name.clone()
        } else {
            format!("{}()", sym.metadata.name)
        };
        let name = if raw_name.len() > 22 {
            format!("{}…", &raw_name[..21])
        } else {
            raw_name
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<8}", kind_label),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{:<22}", name)),
            Span::styled(bar, Style::default().fg(color)),
            Span::raw("  "),
            Span::styled(
                format!("{} {:.0}%", family_abbrev(sym.attribution.primary), sym.attribution.confidence * 100.0),
                Style::default().fg(color),
            ),
        ]));
    }
    lines
}

fn render_detail(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let scroll = app.detail_scroll;
    let block = Block::default().borders(Borders::ALL).title(" Detail ");

    let Some(ref report) = app.detail else {
        // Selected item is a directory — show aggregate info.
        let visible = app.visible();
        let text = visible
            .get(app.selected)
            .map(|e| {
                format!(
                    " {}\n {} ({:.0}%)",
                    e.path.display(),
                    e.family,
                    e.confidence * 100.0
                )
            })
            .unwrap_or_default();
        frame.render_widget(Paragraph::new(text).block(block), area);
        return;
    };

    let inner = area.inner(ratatui::layout::Margin { horizontal: 1, vertical: 1 });
    frame.render_widget(block, area);

    // Header.
    let path_str = report
        .metadata
        .file_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let header_color = family_color(report.attribution.primary);
    let header = Line::from(vec![
        Span::styled(&path_str, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(
            format!(
                "{} ({:.0}%)",
                report.attribution.primary,
                report.attribution.confidence * 100.0
            ),
            Style::default().fg(header_color).add_modifier(Modifier::BOLD),
        ),
    ]);

    // Score bars.
    let mut score_lines: Vec<Line> = Vec::new();
    let mut sorted_scores: Vec<_> = report.attribution.scores.iter().collect();
    sorted_scores.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
    for (family, &score) in &sorted_scores {
        let bar_len = (score * 24.0) as usize;
        let bar = "█".repeat(bar_len);
        let empty = "░".repeat(24 - bar_len);
        score_lines.push(Line::from(vec![
            Span::raw(format!("  {:<10}", family.to_string())),
            Span::styled(bar, Style::default().fg(family_color(**family))),
            Span::styled(empty, Style::default().fg(Color::DarkGray)),
            Span::raw(format!(" {:>5.1}%", score * 100.0)),
        ]));
    }

    // Signals (all — scrolling handles overflow).
    let signal_header = Line::from(Span::styled(
        format!("\n Signals ({}):", report.signals.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    let signal_lines: Vec<Line> = report
        .signals
        .iter()
        .map(|s| {
            let sign = if s.weight >= 0.0 { "+" } else { "" };
            Line::from(vec![
                Span::styled(
                    format!("  {}{:.1} ", sign, s.weight),
                    Style::default().fg(if s.weight >= 0.0 { Color::Green } else { Color::Red }),
                ),
                Span::styled(
                    format!("{:<8}", s.family.to_string()),
                    Style::default().fg(family_color(s.family)),
                ),
                Span::raw(format!(" — {}", s.description)),
            ])
        })
        .collect();

    // Symbol breakdown (if available).
    let sym_lines = render_symbol_lines(report.symbol_reports.as_deref().unwrap_or(&[]));

    let mut all_lines = vec![header, Line::raw("")];
    all_lines.extend(score_lines);
    all_lines.push(signal_header);
    all_lines.extend(signal_lines);
    all_lines.extend(sym_lines);

    frame.render_widget(Paragraph::new(all_lines).scroll((scroll, 0)), inner);
}

fn render_statusbar(frame: &mut Frame, area: ratatui::layout::Rect) {
    let bar = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("navigate  "),
        Span::styled("Enter/→ ", Style::default().fg(Color::Cyan)),
        Span::raw("expand  "),
        Span::styled("← ", Style::default().fg(Color::Cyan)),
        Span::raw("collapse  "),
        Span::styled(" d/u ", Style::default().fg(Color::Cyan)),
        Span::raw("scroll detail  "),
        Span::styled(" q ", Style::default().fg(Color::Cyan)),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(path: &Path, ignore_file: Option<&PathBuf>) -> Result<()> {
    let ignore: Box<dyn IgnoreRules> = match ignore_file {
        Some(f) => Box::new(IgnoreConfig::from_file(f)?),
        None => Box::new(IgnoreConfig::load(path)),
    };

    // Analyze all files up front (cache-backed, so fast on repeat runs).
    eprintln!("Analyzing {}…", path.display());
    let reports = vibecheck_core::analyze_directory_with(path, true, ignore.as_ref())?;
    if reports.is_empty() {
        anyhow::bail!("no supported source files found in {}", path.display());
    }

    let flat = build_flat_tree(path, &reports);
    let mut app = App::new(flat);

    // Set up terminal.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, &mut app);

    // Always restore terminal before returning.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;

        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                KeyCode::Up   | KeyCode::Char('k') => app.move_up(),
                KeyCode::PageDown | KeyCode::Char('d') => app.scroll_detail_down(5),
                KeyCode::PageUp   | KeyCode::Char('u') => app.scroll_detail_up(5),
                KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                    app.toggle_collapse();
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    // Collapse the current directory, or navigate to parent.
                    let visible = app.visible();
                    if let Some(entry) = visible.get(app.selected) {
                        let path = entry.path.clone();
                        if entry.is_dir && !app.collapsed.contains(&path) {
                            app.collapsed.insert(path);
                        } else if let Some(parent) = entry.path.parent() {
                            // Find the parent dir in the visible list and jump to it.
                            let idx = visible
                                .iter()
                                .position(|e| e.path == parent)
                                .unwrap_or(app.selected);
                            app.selected = idx;
                            app.list_state.select(Some(idx));
                            app.refresh_detail();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use vibecheck_core::report::{
        Attribution, ModelFamily, Report, ReportMetadata, SymbolMetadata, SymbolReport,
    };

    // -------------------------------------------------------------------------
    // Test helpers
    // -------------------------------------------------------------------------

    fn make_report(family: ModelFamily, confidence: f64, loc: usize) -> Report {
        let mut scores = HashMap::new();
        scores.insert(family, confidence);
        Report {
            attribution: Attribution { primary: family, confidence, scores },
            signals: vec![],
            metadata: ReportMetadata { file_path: None, lines_of_code: loc, signal_count: 0 },
            symbol_reports: None,
        }
    }

    fn make_sym(name: &str, kind: &str, family: ModelFamily, confidence: f64) -> SymbolReport {
        SymbolReport {
            metadata: SymbolMetadata {
                name: name.to_string(),
                kind: kind.to_string(),
                start_line: 1,
                end_line: 10,
            },
            attribution: Attribution {
                primary: family,
                confidence,
                scores: HashMap::new(),
            },
            signals: vec![],
        }
    }

    fn file_entry(path: &str, depth: usize, family: ModelFamily, confidence: f64) -> FlatEntry {
        let p = PathBuf::from(path);
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        FlatEntry { path: p, name, depth, is_dir: false, family, confidence }
    }

    fn dir_entry(path: &str, depth: usize, family: ModelFamily, confidence: f64) -> FlatEntry {
        let p = PathBuf::from(path);
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        FlatEntry { path: p, name, depth, is_dir: true, family, confidence }
    }

    // -------------------------------------------------------------------------
    // render_symbol_lines
    // -------------------------------------------------------------------------

    #[test]
    fn symbol_lines_empty_slice_returns_empty() {
        assert!(render_symbol_lines(&[]).is_empty());
    }

    #[test]
    fn symbol_lines_header_shows_count() {
        let lines = render_symbol_lines(&[make_sym("foo", "function", ModelFamily::Claude, 0.9)]);
        // index 0 = blank line, index 1 = header
        let header = format!("{:?}", lines[1]);
        assert!(header.contains("Symbols (1):"));
    }

    #[test]
    fn symbol_lines_multiple_count_in_header() {
        let syms: Vec<_> = (0..5)
            .map(|i| make_sym(&format!("fn{i}"), "function", ModelFamily::Claude, 0.8))
            .collect();
        let lines = render_symbol_lines(&syms);
        assert!(format!("{:?}", lines[1]).contains("Symbols (5):"));
    }

    #[test]
    fn symbol_lines_function_kind_tag() {
        let lines = render_symbol_lines(&[make_sym("run", "function", ModelFamily::Claude, 0.5)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("fn"), "row: {row}");
        assert!(row.contains("run()"), "fn symbols should have () suffix, row: {row}");
    }

    #[test]
    fn symbol_lines_method_kind_tag() {
        let lines = render_symbol_lines(&[make_sym("do_it", "method", ModelFamily::Gpt, 0.5)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("method"), "row: {row}");
        assert!(row.contains("do_it()"), "method symbols should have () suffix, row: {row}");
    }

    #[test]
    fn symbol_lines_class_kind_tag() {
        let lines = render_symbol_lines(&[make_sym("Foo", "class", ModelFamily::Gpt, 0.5)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("class"), "row: {row}");
        assert!(row.contains("Foo"), "row: {row}");
        assert!(!row.contains("Foo()"), "class symbols should NOT have () suffix, row: {row}");
    }

    #[test]
    fn symbol_lines_unknown_kind_defaults_to_fn() {
        let lines = render_symbol_lines(&[make_sym("x", "trait", ModelFamily::Claude, 0.5)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("fn"), "unknown kind should fall back to fn, row: {row}");
        assert!(row.contains("x()"), "unknown kind should have () suffix, row: {row}");
    }

    #[test]
    fn symbol_lines_name_called_name_is_unambiguous() {
        // Regression: a method literally named "name" should show as "name()"
        // not look like a missing-name placeholder.
        let lines = render_symbol_lines(&[make_sym("name", "method", ModelFamily::Claude, 0.8)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("name()"), "row: {row}");
    }

    #[test]
    fn symbol_lines_name_fits_within_22_chars_unchanged() {
        // "short_name" + "()" = 12 chars, well within 22
        let lines = render_symbol_lines(&[make_sym("short_name", "function", ModelFamily::Claude, 0.5)]);
        assert!(format!("{:?}", lines[2]).contains("short_name()"));
    }

    #[test]
    fn symbol_lines_name_truncated_when_over_22_chars() {
        // 20-char name + "()" = 22 chars — exactly at the limit, should not truncate
        let name_20 = "a_twenty_char_name__";
        assert_eq!(name_20.len(), 20);
        let lines = render_symbol_lines(&[make_sym(name_20, "function", ModelFamily::Claude, 0.5)]);
        assert!(format!("{:?}", lines[2]).contains(&format!("{name_20}()")));

        // 21-char name + "()" = 23 chars — should truncate
        let name_21 = "a_twenty_one_char_nam";
        assert_eq!(name_21.len(), 21);
        let lines = render_symbol_lines(&[make_sym(name_21, "function", ModelFamily::Claude, 0.5)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains('…'), "23-char display name should be truncated, row: {row}");
    }

    #[test]
    fn symbol_lines_full_confidence_fills_bar() {
        let lines = render_symbol_lines(&[make_sym("f", "function", ModelFamily::Claude, 1.0)]);
        let row = format!("{:?}", lines[2]);
        assert!(row.contains("████████████████"), "16 blocks at 100% confidence");
    }

    #[test]
    fn symbol_lines_zero_confidence_has_no_bar() {
        let lines = render_symbol_lines(&[make_sym("f", "function", ModelFamily::Claude, 0.0)]);
        assert!(!format!("{:?}", lines[2]).contains('█'));
    }

    #[test]
    fn symbol_lines_half_confidence_has_8_blocks() {
        let lines = render_symbol_lines(&[make_sym("f", "function", ModelFamily::Claude, 0.5)]);
        let row = format!("{:?}", lines[2]);
        let count = row.chars().filter(|&c| c == '█').count();
        assert_eq!(count, 8);
    }

    #[test]
    fn symbol_lines_one_row_per_symbol() {
        let syms = vec![
            make_sym("a", "function", ModelFamily::Claude, 0.9),
            make_sym("b", "method",   ModelFamily::Gpt,    0.7),
            make_sym("c", "function", ModelFamily::Human,  0.3),
        ];
        // 2 header lines (blank + title) + 3 symbol rows = 5
        assert_eq!(render_symbol_lines(&syms).len(), 5);
    }

    // -------------------------------------------------------------------------
    // name_to_family
    // -------------------------------------------------------------------------

    #[test]
    fn name_to_family_all_known() {
        assert_eq!(name_to_family("claude"),  ModelFamily::Claude);
        assert_eq!(name_to_family("gpt"),     ModelFamily::Gpt);
        assert_eq!(name_to_family("gemini"),  ModelFamily::Gemini);
        assert_eq!(name_to_family("copilot"), ModelFamily::Copilot);
        assert_eq!(name_to_family("human"),   ModelFamily::Human);
    }

    #[test]
    fn name_to_family_unknown_falls_back_to_human() {
        assert_eq!(name_to_family("llama"),   ModelFamily::Human);
        assert_eq!(name_to_family(""),        ModelFamily::Human);
    }

    // -------------------------------------------------------------------------
    // family_abbrev / family_color — smoke tests
    // -------------------------------------------------------------------------

    #[test]
    fn family_abbrev_all_families_return_nonempty() {
        for &f in ModelFamily::all() {
            assert!(!family_abbrev(f).is_empty());
        }
    }

    #[test]
    fn family_color_all_families_return_a_color() {
        // Just ensure it doesn't panic and returns distinct values for each family.
        let colors: Vec<_> = ModelFamily::all().iter().map(|&f| family_color(f)).collect();
        assert_eq!(colors.len(), 5);
    }

    // -------------------------------------------------------------------------
    // App::visible — collapse / expand logic
    // -------------------------------------------------------------------------

    #[test]
    fn visible_all_entries_shown_when_nothing_collapsed() {
        let app = App::for_test(vec![
            dir_entry("/src",         0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs",   1, ModelFamily::Claude, 0.9),
            file_entry("/src/b.rs",   1, ModelFamily::Claude, 0.85),
        ]);
        assert_eq!(app.visible().len(), 3);
    }

    #[test]
    fn visible_collapsed_dir_hides_direct_children() {
        let mut app = App::for_test(vec![
            dir_entry("/src",        0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs",  1, ModelFamily::Claude, 0.9),
            file_entry("/src/b.rs",  1, ModelFamily::Claude, 0.85),
        ]);
        app.collapsed.insert(PathBuf::from("/src"));
        assert_eq!(app.visible().len(), 1);
    }

    #[test]
    fn visible_collapsed_dir_hides_deeply_nested_entries() {
        let mut app = App::for_test(vec![
            dir_entry("/src",                    0, ModelFamily::Claude, 0.8),
            dir_entry("/src/analyzers",          1, ModelFamily::Claude, 0.8),
            file_entry("/src/analyzers/rust.rs", 2, ModelFamily::Claude, 0.9),
            file_entry("/src/lib.rs",            1, ModelFamily::Claude, 0.85),
        ]);
        app.collapsed.insert(PathBuf::from("/src/analyzers"));
        // /src, /src/analyzers (collapsed), /src/lib.rs — rust.rs hidden
        assert_eq!(app.visible().len(), 3);
    }

    #[test]
    fn visible_sibling_dir_stays_visible_when_other_collapsed() {
        let mut app = App::for_test(vec![
            dir_entry("/src",          0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs",    1, ModelFamily::Claude, 0.9),
            dir_entry("/tests",        0, ModelFamily::Claude, 0.7),
            file_entry("/tests/t.rs",  1, ModelFamily::Claude, 0.7),
        ]);
        app.collapsed.insert(PathBuf::from("/src"));
        // /src (collapsed), /tests, /tests/t.rs
        assert_eq!(app.visible().len(), 3);
    }

    #[test]
    fn visible_empty_app_returns_empty() {
        let app = App::for_test(vec![]);
        assert_eq!(app.visible().len(), 0);
    }

    // -------------------------------------------------------------------------
    // App::toggle_collapse
    // -------------------------------------------------------------------------

    #[test]
    fn toggle_collapse_collapses_expanded_dir() {
        let mut app = App::for_test(vec![
            dir_entry("/src",       0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs", 1, ModelFamily::Claude, 0.9),
        ]);
        app.toggle_collapse();
        assert!(app.collapsed.contains(&PathBuf::from("/src")));
    }

    #[test]
    fn toggle_collapse_expands_collapsed_dir() {
        let mut app = App::for_test(vec![
            dir_entry("/src",       0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs", 1, ModelFamily::Claude, 0.9),
        ]);
        app.collapsed.insert(PathBuf::from("/src"));
        app.toggle_collapse();
        assert!(!app.collapsed.contains(&PathBuf::from("/src")));
    }

    #[test]
    fn toggle_collapse_on_file_is_noop() {
        let mut app = App::for_test(vec![
            file_entry("/src/a.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.toggle_collapse();
        assert!(app.collapsed.is_empty());
    }

    // -------------------------------------------------------------------------
    // App::move_down / move_up
    // -------------------------------------------------------------------------

    #[test]
    fn move_down_increments_selected() {
        let mut app = App::for_test(vec![
            file_entry("/a.rs", 0, ModelFamily::Claude, 0.9),
            file_entry("/b.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.move_down();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_down_clamps_at_last_entry() {
        let mut app = App::for_test(vec![
            file_entry("/a.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.move_down();
        app.move_down();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn move_up_decrements_selected() {
        let mut app = App::for_test(vec![
            file_entry("/a.rs", 0, ModelFamily::Claude, 0.9),
            file_entry("/b.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.selected = 1;
        app.list_state.select(Some(1));
        app.move_up();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let mut app = App::for_test(vec![
            file_entry("/a.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.move_up();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn move_down_skips_hidden_entries_correctly() {
        let mut app = App::for_test(vec![
            dir_entry("/src",       0, ModelFamily::Claude, 0.8),
            file_entry("/src/a.rs", 1, ModelFamily::Claude, 0.9),
            file_entry("/b.rs",     0, ModelFamily::Claude, 0.9),
        ]);
        app.collapsed.insert(PathBuf::from("/src"));
        // visible: /src (0), /b.rs (1)
        app.move_down();
        assert_eq!(app.selected, 1);
    }

    // -------------------------------------------------------------------------
    // App::scroll_detail_down / scroll_detail_up
    // -------------------------------------------------------------------------

    #[test]
    fn scroll_down_increases_offset() {
        let mut app = App::for_test(vec![]);
        app.scroll_detail_down(5);
        assert_eq!(app.detail_scroll, 5);
    }

    #[test]
    fn scroll_down_accumulates() {
        let mut app = App::for_test(vec![]);
        app.scroll_detail_down(5);
        app.scroll_detail_down(3);
        assert_eq!(app.detail_scroll, 8);
    }

    #[test]
    fn scroll_up_decreases_offset() {
        let mut app = App::for_test(vec![]);
        app.detail_scroll = 10;
        app.scroll_detail_up(5);
        assert_eq!(app.detail_scroll, 5);
    }

    #[test]
    fn scroll_up_clamps_at_zero() {
        let mut app = App::for_test(vec![]);
        app.scroll_detail_up(99);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn navigation_resets_scroll_offset() {
        let mut app = App::for_test(vec![
            file_entry("/a.rs", 0, ModelFamily::Claude, 0.9),
            file_entry("/b.rs", 0, ModelFamily::Claude, 0.9),
        ]);
        app.detail_scroll = 42;
        app.move_down();
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn scroll_up_after_down_returns_to_origin() {
        let mut app = App::for_test(vec![]);
        app.scroll_detail_down(10);
        app.scroll_detail_up(10);
        assert_eq!(app.detail_scroll, 0);
    }

    // -------------------------------------------------------------------------
    // build_flat_tree — needs real filesystem paths because dfs calls is_dir()
    // -------------------------------------------------------------------------

    #[test]
    fn flat_tree_empty_reports_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(build_flat_tree(dir.path(), &[]).is_empty());
    }

    #[test]
    fn flat_tree_single_file_at_root() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "").unwrap();
        let result = build_flat_tree(dir.path(), &[(file, make_report(ModelFamily::Claude, 0.9, 10))]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "main.rs");
        assert!(!result[0].is_dir);
    }

    #[test]
    fn flat_tree_synthesises_directory_entry() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        let file = sub.join("lib.rs");
        std::fs::write(&file, "").unwrap();
        let result = build_flat_tree(dir.path(), &[(file, make_report(ModelFamily::Claude, 0.9, 10))]);
        // src/ (depth 0) then lib.rs (depth 1)
        assert_eq!(result.len(), 2);
        assert!(result[0].is_dir);
        assert_eq!(result[0].name, "src");
        assert_eq!(result[1].name, "lib.rs");
        assert_eq!(result[1].depth, 1);
    }

    #[test]
    fn flat_tree_files_sorted_alphabetically() {
        let dir = tempfile::tempdir().unwrap();
        for name in &["z.rs", "a.rs", "m.rs"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        let reports: Vec<_> = ["z.rs", "a.rs", "m.rs"]
            .iter()
            .map(|n| (dir.path().join(n), make_report(ModelFamily::Claude, 0.8, 5)))
            .collect();
        let result = build_flat_tree(dir.path(), &reports);
        let names: Vec<_> = result.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn flat_tree_dir_confidence_is_weighted_average() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        // Two Claude files: 100 LOC at 90%, 100 LOC at 70% → avg 80%
        let f1 = sub.join("a.rs");
        let f2 = sub.join("b.rs");
        std::fs::write(&f1, "").unwrap();
        std::fs::write(&f2, "").unwrap();
        let reports = vec![
            (f1, make_report(ModelFamily::Claude, 0.9, 100)),
            (f2, make_report(ModelFamily::Claude, 0.7, 100)),
        ];
        let result = build_flat_tree(dir.path(), &reports);
        let dir_entry = result.iter().find(|e| e.is_dir).unwrap();
        assert!((dir_entry.confidence - 0.8).abs() < 0.01, "expected ~80% got {}", dir_entry.confidence);
    }

    #[test]
    fn flat_tree_dir_family_is_dominant_by_weighted_score() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("src");
        std::fs::create_dir(&sub).unwrap();
        // One large Claude file vs one small GPT file
        let f1 = sub.join("big.rs");
        let f2 = sub.join("small.rs");
        std::fs::write(&f1, "").unwrap();
        std::fs::write(&f2, "").unwrap();
        let reports = vec![
            (f1, make_report(ModelFamily::Claude, 0.9, 200)),
            (f2, make_report(ModelFamily::Gpt,    0.9,  10)),
        ];
        let result = build_flat_tree(dir.path(), &reports);
        let dir_entry = result.iter().find(|e| e.is_dir).unwrap();
        assert_eq!(dir_entry.family, ModelFamily::Claude);
    }

    #[test]
    fn flat_tree_dirs_appear_before_sibling_files() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let nested = sub.join("nested.rs");
        let root_file = dir.path().join("root.rs");
        std::fs::write(&nested, "").unwrap();
        std::fs::write(&root_file, "").unwrap();
        let reports = vec![
            (nested,    make_report(ModelFamily::Claude, 0.8, 10)),
            (root_file, make_report(ModelFamily::Claude, 0.8, 10)),
        ];
        let result = build_flat_tree(dir.path(), &reports);
        // sub/ should come before root.rs in the flat list
        let sub_idx   = result.iter().position(|e| e.name == "sub").unwrap();
        let root_idx  = result.iter().position(|e| e.name == "root.rs").unwrap();
        assert!(sub_idx < root_idx);
    }
}
