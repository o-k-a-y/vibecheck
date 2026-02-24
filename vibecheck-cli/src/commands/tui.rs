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
}

impl App {
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
        let kind_tag = match sym.metadata.kind.as_str() {
            "method" => "M",
            "class"  => "C",
            _        => "f",
        };
        let name = if sym.metadata.name.len() > 22 {
            format!("{}…", &sym.metadata.name[..21])
        } else {
            sym.metadata.name.clone()
        };
        lines.push(Line::from(vec![
            Span::raw(format!("  {kind_tag} {:<23}", name)),
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

    // Signals (up to what fits).
    let signal_header = Line::from(Span::styled(
        format!("\n Signals ({}):", report.signals.len()),
        Style::default().add_modifier(Modifier::BOLD),
    ));
    let max_signals = inner.height.saturating_sub(5 + sorted_scores.len() as u16 + 2) as usize;
    let signal_lines: Vec<Line> = report
        .signals
        .iter()
        .take(max_signals)
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

    frame.render_widget(Paragraph::new(all_lines), inner);
}

fn render_statusbar(frame: &mut Frame, area: ratatui::layout::Rect) {
    let bar = Paragraph::new(Line::from(vec![
        Span::styled(" ↑↓ ", Style::default().fg(Color::Cyan)),
        Span::raw("navigate  "),
        Span::styled("Enter/→ ", Style::default().fg(Color::Cyan)),
        Span::raw("expand  "),
        Span::styled("← ", Style::default().fg(Color::Cyan)),
        Span::raw("collapse  "),
        Span::styled(" q ", Style::default().fg(Color::Cyan)),
        Span::raw("quit"),
    ]))
    .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(bar, area);
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(path: &Path) -> Result<()> {
    // Analyze all files up front (cache-backed, so fast on repeat runs).
    eprintln!("Analyzing {}…", path.display());
    let reports = vibecheck_core::analyze_directory(path, true)?;
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
                KeyCode::Up | KeyCode::Char('k') => app.move_up(),
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
