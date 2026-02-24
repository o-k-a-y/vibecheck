# vibecheck

<p align="center">
  <img src="https://raw.githubusercontent.com/o-k-a-y/vibecheck/main/.github/assets/logo.svg" alt="vibecheck" />
</p>

[![CI](https://github.com/o-k-a-y/vibecheck/actions/workflows/vibecheck.yml/badge.svg)](https://github.com/o-k-a-y/vibecheck/actions/workflows/vibecheck.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/o-k-a-y/vibecheck/blob/main/LICENSE)
[![Rust 2021](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)
[![vibecheck-core on crates.io](https://img.shields.io/crates/v/vibecheck-core.svg?label=vibecheck-core)](https://crates.io/crates/vibecheck-core)
[![vibecheck-cli on crates.io](https://img.shields.io/crates/v/vibecheck-cli.svg?label=vibecheck-cli)](https://crates.io/crates/vibecheck-cli)
[![vibecheck: Claude 96%](https://img.shields.io/badge/vibecheck-Claude%2096%25-a855f7)](https://github.com/o-k-a-y/vibecheck)

> *"I don't always write Rust, but when I do, every function has a doc comment and zero `.unwrap()` calls."*
> *— The Most Interesting LLM in the World*

**vibecheck** detects AI-generated code and attributes it to a model family. It sniffs out the telltale "vibes" that different AI models leave in code — the suspiciously perfect formatting, the teaching-voice comments, the conspicuous absence of `TODO: fix this later`.

![vibecheck example output](https://raw.githubusercontent.com/o-k-a-y/vibecheck/main/.github/assets/example.svg)

```
   The 5 stages of vibecheck grief:

   1. Denial     "I wrote this myself"
   2. Anger      "The heuristics are WRONG"
   3. Bargaining "Ok but I modified 2 lines"
   4. Depression  vibecheck src/my_code.rs
                  > Verdict: Claude (94%)
   5. Acceptance "...yeah that's fair"

   ───────────────────────────────────────

   Nobody:
   Absolutely nobody:
   Your AI-generated code:

      /// Processes the input data by applying the configured
      /// transformation pipeline and returning the validated result.
      pub fn process_and_validate_input_data(
          &self,
          input_data: &InputData,
      ) -> Result<ValidatedOutput, ProcessingError> {
```

## How It Works

vibecheck runs your source code through two layers of analysis:

**Layer 1 — Text-pattern analyzers** (all languages):

| Analyzer | What It Sniffs | Example Signal |
|----------|---------------|----------------|
| **Comment Style** | Density, teaching voice, doc comments | *"12 comments with teaching/explanatory voice"* |
| **AI Signals** | TODO absence, no dead code, eerie perfection | *"Every function has a doc comment — suspiciously thorough"* |
| **Error Handling** | unwrap vs expect vs ?, panic usage | *"Zero .unwrap() calls — careful error handling"* |
| **Naming** | Variable length, descriptiveness, single-char names | *"Very descriptive variable names (avg 14.2 chars)"* |
| **Code Structure** | Type annotations, import ordering, formatting | *"Import statements are alphabetically sorted"* |
| **Idiom Usage** | Iterator chains, builder patterns, Display impls | *"8 iterator chain usages — textbook-idiomatic Rust"* |

**Layer 2 — tree-sitter CST analyzers** (language-aware):

| Language | Signals |
|----------|---------|
| **Rust** | Cyclomatic complexity, doc comment coverage on pub fns, identifier entropy, nesting depth, import ordering |
| **Python** | Docstring coverage, type annotation coverage, f-string vs %-format ratio |
| **JavaScript** | Arrow function ratio, async/await vs `.then()` chaining, optional chaining density |
| **Go** | Godoc coverage on exported functions, goroutine count, `err != nil` check density |

Each signal has a **weight** (positive = evidence for, negative = evidence against) and points to a **model family**. The pipeline aggregates all signals into a probability distribution.

Results are stored in a **content-addressed cache** (redb, keyed by SHA-256 of file contents) so unchanged files are never re-analyzed. A **Merkle hash tree** extends this to directory level — unchanged subdirectories are skipped entirely, making repeated directory scans near-instant.

## Installation

```bash
# Install the CLI
cargo install vibecheck-cli

# Add the library to your project
cargo add vibecheck-core
```

## Usage

### CLI

```bash
# No arguments: opens the TUI browser in the current directory
vibecheck

# Analyze a single file (pretty output with colors)
vibecheck src/main.rs

# Analyze a directory (supports .rs, .py, .js, .ts, .go)
vibecheck src/

# Symbol-level attribution — breaks down each function/method individually
vibecheck --symbols src/main.rs

# Plain text output
vibecheck src/lib.rs --format text

# JSON output (for piping to other tools)
vibecheck src/ --format json

# Enforce attribution in CI — exit 1 if any file isn't attributed to one of these families
vibecheck src/ --assert-family claude,gpt,copilot,gemini

# Assert human authorship specifically
vibecheck src/ --assert-family human

# Skip the cache (always re-analyze, useful for CI reproducibility)
vibecheck src/ --no-cache

# List all detection signals with their default weights (pretty table)
vibecheck heuristics

# Same list as a TOML block ready to paste into .vibecheck
vibecheck heuristics --format toml
```

All commands are also available as explicit subcommands: `vibecheck analyze`, `vibecheck tui`, `vibecheck watch`, `vibecheck history`.

`--assert-family` accepts a comma-separated list of `claude`, `gpt`, `copilot`, `gemini`, or `human`. If any analyzed file's primary attribution is **not** in the list, vibecheck prints a failure summary to stderr and exits with code `1`. This is the flag that makes vibecheck useful in CI.

### TUI Codebase Navigator

```bash
# Open TUI in the current directory (same as running vibecheck with no args)
vibecheck

# Or point at a specific directory
vibecheck tui src/
```

![vibecheck TUI screenshot](https://raw.githubusercontent.com/o-k-a-y/vibecheck/main/.github/assets/tui.svg)

Two-pane browser: file tree with family badges on the left, signal/score/symbol breakdown on the right. Confidence rolls up from symbol → file → directory (weighted by lines of code).

| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` / `→` / `l` | Expand directory |
| `←` | Collapse directory or go to parent |
| `d` / `PageDown` | Scroll detail pane down |
| `u` / `PageUp` | Scroll detail pane up |
| `⇧→` / `⇧←` | Scroll detail pane right / left |
| `h` | Toggle git history panel (files only) |
| `↑` / `↓` in history | Navigate commits |
| `Esc` | Close history panel |
| `q` / `Ctrl+C` | Quit |

### Live Watch Mode

```bash
# Re-analyze on every file save, print deltas to stdout
vibecheck watch src/
```

Uses OS file-system events (inotify/kqueue/FSEvents) with a 300 ms debounce and a 2 s per-file cooldown to suppress duplicate events from a single save.

### Ignore Rules

vibecheck respects `.gitignore` automatically. For additional exclusions, drop a `.vibecheck` file in your project root:

```toml
# .vibecheck
[ignore]
# Extra patterns (gitignore glob syntax), additive on top of .gitignore.
patterns = [
  "vendor/",
  "dist/",
  "*.min.js",
  "*.generated.ts",
]

# Set to false to stop reading .gitignore (default: true).
use_gitignore = true

# Set to false to stop reading the global gitignore (default: true).
use_global_gitignore = true
```

Discovery walks upward from the analyzed path to the nearest `.vibecheck` file or `.git` directory. Falls back to gitignore-only if no config file is found.

To point at a config file explicitly on any subcommand:

```bash
vibecheck src/ --ignore-file path/to/.vibecheck
vibecheck tui src/ --ignore-file path/to/.vibecheck
vibecheck watch src/ --ignore-file path/to/.vibecheck
```

Ignored paths are excluded from all traversal layers — they do not enter the file list, the Merkle hash tree, or the watch event queue.

### Heuristics

Every detection rule in vibecheck is a **signal** with three properties:

- **Stable ID** (`rust.errors.zero_unwrap`) — used as the config key and for cache invalidation
- **Weight** — how strongly the signal shifts the score (positive = evidence for the family; `0.0` = disabled)
- **Family** — which model family the signal points toward (Claude, Gpt, Copilot, Human, …)

There are currently 151 signals across Rust, Python, JavaScript, and Go.

#### Viewing signals

```bash
# Pretty table grouped by language then analyzer (default)
vibecheck heuristics

# Output:
# Language  Analyzer    Signal ID                     Family  Weight  Description
# ─────────────────────────────────────────────────────────────────────────────
# rust      errors      rust.errors.zero_unwrap       Claude  1.50    Zero .unwrap() calls in a large file
# rust      errors      rust.errors.many_unwraps      Human   1.50    5+ .unwrap() calls — pragmatic style
# …

# TOML block ready to paste into .vibecheck
vibecheck heuristics --format toml

# Output:
# [heuristics]
# # "rust.errors.zero_unwrap" = 1.5   # Claude: Zero .unwrap() calls in a large file
# # "rust.errors.many_unwraps" = 1.5  # Human:  5+ .unwrap() calls — pragmatic style
# …
```

#### Overriding weights

Add a `[heuristics]` section to your `.vibecheck` config. Any signal not listed falls back to its default weight.

```toml
# .vibecheck
[ignore]
patterns = ["vendor/", "dist/"]

[heuristics]
# Double the zero-unwrap signal — you care a lot about this one
"rust.errors.zero_unwrap" = 3.0

# Disable the trailing-whitespace signal — your auto-formatter isn't deterministic
"rust.ai_signals.no_trailing_ws" = 0.0

# Your codebase uses panic! legitimately; reduce human penalty
"rust.errors.panic_calls" = 0.5
```

Setting a weight to `0.0` **disables** the signal entirely — it won't appear in reports or affect scores. Weights above the default amplify a signal you find particularly reliable.

Run `vibecheck heuristics --format toml` to get a pre-commented block of every signal with its default — copy, uncomment, and edit.

#### Signal catalogue

A representative sample (run `vibecheck heuristics` for the full 151-signal table):

| Language | Analyzer | Signal ID | Family | Weight | Description |
|----------|----------|-----------|--------|--------|-------------|
| rust | errors | `rust.errors.zero_unwrap` | Claude | 1.5 | Zero `.unwrap()` calls in a large file |
| rust | errors | `rust.errors.many_unwraps` | Human | 1.5 | 5+ `.unwrap()` calls — pragmatic style |
| rust | errors | `rust.errors.panic_calls` | Human | 1.5 | 2+ `panic!()` calls |
| rust | ai_signals | `rust.ai_signals.all_fns_documented` | Claude | 2.0 | Every function has a doc comment — suspiciously thorough |
| rust | ai_signals | `rust.ai_signals.commented_out_code` | Human | 2.5 | 2+ lines of commented-out code |
| rust | naming | `rust.naming.very_descriptive_vars` | Claude | 1.5 | Very descriptive variable names (avg >12 chars) |
| rust | naming | `rust.naming.many_single_char_vars` | Human | 2.0 | 3+ single-character variable names |
| rust | idioms | `rust.idioms.iterator_chains` | Claude | 1.5 | 5+ iterator chain usages — textbook-idiomatic Rust |
| rust | idioms | `rust.idioms.string_concat` | Human | 1.0 | 3+ string concatenations — less idiomatic |
| rust_cst | rust_cst | `rust_cst.complexity.low` | Claude | 2.5 | Low avg cyclomatic complexity (≤2.0) — simple, linear functions |
| rust_cst | rust_cst | `rust_cst.doc_coverage.high` | Claude | 2.0 | ≥90% doc comment coverage on pub functions |
| rust_cst | rust_cst | `rust_cst.nesting.low` | Claude | 1.5 | Low avg nesting depth (≤3.0) — flat, readable structure |
| python | errors | `python.errors.broad_except` | Human | 1.5 | 2+ broad `except` clauses — swallows all exceptions |
| python | ai_signals | `python.ai_signals.all_fns_documented` | Claude | 2.0 | Every function has a docstring — suspiciously thorough |
| python | idioms | `python.idioms.comprehensions` | Claude | 1.5 | 3+ list/dict/set comprehensions — pythonic style |
| python | idioms | `python.idioms.old_format` | Human | 1.0 | 3+ old-style `%`-format calls — legacy string formatting |
| python_cst | python_cst | `python_cst.doc_coverage.high` | Claude | 2.0 | ≥85% docstring coverage — thorough documentation |
| python_cst | python_cst | `python_cst.type_annotations.high` | Claude | 1.5 | ≥80% type annotation coverage on parameters |
| js | idioms | `js.idioms.arrow_fns_only` | Claude | 1.5 | 5+ arrow functions, no regular functions — modern ES6+ |
| js | idioms | `js.idioms.var_declarations` | Human | 1.5 | 3+ `var` declarations — legacy hoisting style |
| js | ai_signals | `js.ai_signals.commented_out_code` | Human | 2.5 | 2+ lines of commented-out code |
| js_cst | js_cst | `js_cst.arrow_fns.high_ratio` | Claude | 1.5 | ≥70% arrow functions — modern JavaScript style |
| go | idioms | `go.idioms.table_driven_tests` | Claude | 1.5 | Table-driven test pattern detected — idiomatic Go testing |
| go | errors | `go.errors.errorf_wrap` | Claude | 1.0 | 2+ `fmt.Errorf(%w)` wrappings — idiomatic error context |
| go | ai_signals | `go.ai_signals.commented_out_code` | Human | 2.5 | 2+ lines of commented-out code |
| go_cst | go_cst | `go_cst.doc_coverage.high` | Claude | 2.0 | ≥80% Godoc coverage on exported functions |

### Git History

```bash
# Replay git history for a file and show how attribution changed over commits
vibecheck history src/pipeline.rs

# Limit to the last N commits that touched the file (default: 20)
vibecheck history src/pipeline.rs --limit 10
```

Reads blobs directly from the git object store (no working-tree checkout). Prints a table: `COMMIT | DATE | FAMILY | CONFIDENCE | CHANGE`.

### Example Output

Not every file is a slam dunk. `src/pipeline.rs` scores 72% — the two `.unwrap()` calls bleed a few points toward Copilot:

```
$ vibecheck src/pipeline.rs

File: src/pipeline.rs
Verdict: Claude (72% confidence)
Lines: 86 | Signals: 12

Scores:
  Claude     █████████████████████ 72.5%
  GPT        ██████ 22.9%
  Copilot    █ 4.6%
  Gemini     0.0%
  Human      0.0%

Signals:
  [ai_signals] +1.5 Claude — No TODO/FIXME markers in a substantial file
  [ai_signals] +0.8 Claude — No dead code suppressions
  [ai_signals] +0.5 GPT — Zero trailing whitespace — machine-perfect formatting
  [errors] +0.5 Copilot — 2 .unwrap() calls — moderate
  [naming] +1.0 Claude — No single-character variable names
  [idioms] +1.5 Claude — 6 iterator chain usages — textbook-idiomatic Rust
  [idioms] +1.0 GPT — 11 method chain continuation lines — builder pattern
  [structure] +1.0 Claude — Import statements are alphabetically sorted
  [structure] +0.8 Claude — All lines under 100 chars — disciplined formatting
  [rust_cst] +2.5 Claude — Low average cyclomatic complexity (1.2) — simple, linear functions
  [rust_cst] +1.5 Claude — Low average nesting depth (2.1) — flat, readable structure
  [rust_cst] +1.0 Claude — use declarations are alphabetically sorted
```

### The Ultimate Test: Self-Detection

vibecheck was written by an AI. Does it know?

```
$ vibecheck vibecheck-core/src/ --format text

vibecheck-core/src/report.rs          → Claude (96%)   # 👀
vibecheck-core/src/cache.rs           → Claude (96%)
vibecheck-core/src/language.rs        → Claude (93%)
vibecheck-core/src/analyzers/cst/python.rs → Claude (85%)
vibecheck-core/src/pipeline.rs        → Claude (74%)   # two .unwrap()s cost it
```

Every file in the codebase is correctly attributed to Claude. The confidence ranges from 74% to 96% depending on how "perfect" the individual file is.

```
$ vibecheck vibecheck-core/src/ --assert-family claude --no-cache

All files passed the vibe check.      # exits 0
```

```
  When the AI detector you wrote with AI detects itself as AI:

            ┌────────────────────────┐
            │                        │
            │   ◉_◉                  │
            │                        │
            │   ...well, well, well. │
            │                        │
            │   If it isn't the      │
            │   consequences of my   │
            │   own architecture.    │
            │                        │
            └────────────────────────┘

  "I'm in this photo and I don't like it"
            — this crate's source code, literally
```

### Library API

```rust
use std::path::Path;
use vibecheck_core::report::ModelFamily;

// Analyze a source string directly (no file I/O)
let report = vibecheck_core::analyze(source_code);
println!("Verdict: {} ({:.0}%)",
    report.attribution.primary,
    report.attribution.confidence * 100.0);

// Analyze a file — content-addressed cache is consulted automatically
// Returns std::io::Result<Report>
let report = vibecheck_core::analyze_file(Path::new("suspect.rs"))?;
if report.attribution.primary != ModelFamily::Human {
    println!("Caught one! Probably written by {}", report.attribution.primary);
}

// Bypass the cache entirely
let report = vibecheck_core::analyze_file_no_cache(Path::new("suspect.rs"))?;

// Symbol-level attribution — Report.symbol_reports is populated
// Returns anyhow::Result<Report>
let report = vibecheck_core::analyze_file_symbols(Path::new("suspect.rs"))?;
if let Some(symbols) = &report.symbol_reports {
    for sym in symbols {
        println!("  {} {}() → {} ({:.0}%)",
            sym.metadata.kind,
            sym.metadata.name,
            sym.attribution.primary,
            sym.attribution.confidence * 100.0);
    }
}

// Symbol-level, cache bypassed
let report = vibecheck_core::analyze_file_symbols_no_cache(Path::new("suspect.rs"))?;

// Directory analysis — Merkle tree skips unchanged subtrees when use_cache=true
// Returns anyhow::Result<Vec<(PathBuf, Report)>>
let results = vibecheck_core::analyze_directory(Path::new("src/"), true)?;
for (path, report) in results {
    println!("{} → {} ({:.0}%)",
        path.display(),
        report.attribution.primary,
        report.attribution.confidence * 100.0);
}

// Directory analysis with custom ignore rules (dependency injection)
use vibecheck_core::ignore_rules::{IgnoreConfig, IgnoreRules, PatternIgnore};

// Production: auto-discover .vibecheck + .gitignore
let ignore = IgnoreConfig::load(Path::new("src/"));
let results = vibecheck_core::analyze_directory_with(Path::new("src/"), true, &ignore)?;

// Load from an explicit config file
let ignore = IgnoreConfig::from_file(Path::new("/project/.vibecheck"))?;
let results = vibecheck_core::analyze_directory_with(Path::new("src/"), true, &ignore)?;

// Tests: inject a lightweight in-memory impl — no filesystem access needed
let ignore = PatternIgnore(vec!["vendor".into(), "dist".into()]);
let results = vibecheck_core::analyze_directory_with(Path::new("src/"), false, &ignore)?;

// Or implement the trait directly for full control
struct MyIgnore;
impl IgnoreRules for MyIgnore {
    fn is_ignored(&self, path: &std::path::Path) -> bool {
        path.to_string_lossy().contains("generated")
    }
}
let results = vibecheck_core::analyze_directory_with(Path::new("src/"), true, &MyIgnore)?;
```

### GitHub Action / CI Integration

A ready-to-use workflow lives at `.github/workflows/vibecheck.yml`. It triggers on every pull request and exits `1` if any file's attribution isn't in the allowed list — blocking the PR automatically.

**Use case 1: enforce that all code is AI-generated** (vibecheck dogfoods this on itself)

```yaml
- name: Vibecheck source code
  run: cargo run --release -p vibecheck-cli -- vibecheck-core/src/ --format text --assert-family claude,gpt,copilot,gemini --no-cache
```

**Use case 2: enforce that all code is human-written** (block AI slop from landing)

```yaml
- name: No AI slop allowed
  run: vibecheck src/ --assert-family human
```

When a file fails, stderr shows exactly what was caught and why:

```
--- VIBECHECK FAILED ---
  src/new_feature.rs — detected as Claude (89%), expected one of: human
```

Exit code `1` fails the job and blocks the PR. Both use cases work the same way — `--assert-family` is just a comma-separated list of families you're willing to accept.

## Architecture

### Current — Multi-Layer Analysis + Incremental Cache

```
                    ┌───────────────────────────────────────┐
                    │           vibecheck-core              │
                    │                                       │
  directory ──────► │  Merkle tree walk                     │
  (.rs/.py/etc.)    │    │ unchanged subtree? skip entirely │
                    │    ▼ changed file: SHA-256 lookup     │
                    │  redb cache (3 tables)                │
                    │    file_cache  │  hit → Report        │
                    │    sym_cache   │  hit → SymbolReports │
                    │    dir_cache   │  hit → DirNode hash  │
                    │                ▼ miss: analyze        │
                    │  TextAnalyzers[]   CstAnalyzers[]     │
                    │   (6 pattern)    (tree-sitter)        │
                    │        └──────────┬──────────┘        │
                    │                Signals                │
                    │                   │                   │
                    │          Aggregate + Normalize        │
                    │                   │                   │
                    │     Report ──────────────► cache.put  │
                    │     SymbolReport[] ───────► sym_cache │
                    └───────────────────┼───────────────────┘
                                        │
                               vibecheck-cli
                     ┌─────────────────┼──────────────────┐
                     │                 │                  │
              analyze / --symbols   tui <path>      watch / history
              (file + dir)         (ratatui TUI)    (notify / git2)
```

**Crate split:**

| Crate | Contents | Who uses it |
|-------|----------|-------------|
| `vibecheck-core` | Analysis engine, CST analyzers, cache, corpus store | any tool that imports it |
| `vibecheck-cli` | CLI binary | end users |

`vibecheck-core` has no CLI dependencies — it is a clean library crate that any tool can import.

## Model Family Profiles

How vibecheck tells them apart:

- **Claude**: Thorough doc comments, teaching voice, zero `unwrap()`, textbook iterator chains, `format!()` over concatenation, sorted imports, suspiciously complete
- **GPT**: Explicit type annotations, builder patterns, method chaining, explanatory (but less pedagogical) comments
- **Copilot**: Works but cuts corners — moderate `unwrap()` usage, less documentation, pragmatic completion style
- **Gemini**: Currently limited signal set (future improvement area)
- **Human**: TODOs everywhere, `// HACK`, commented-out code, single-character variables, `panic!()` calls, string concatenation, chaotic formatting

## Feature Flags

| Crate | Feature | Default | What it enables |
|-------|---------|---------|-----------------|
| `vibecheck-core` | `corpus` | No | SQLite corpus + trend store (`rusqlite`) |
| `vibecheck-cli` | — | — | CLI binary; always has `clap`, `walkdir`, `colored`, `anyhow` |

### The `corpus` feature

The corpus store is separate from the content-addressed redb cache. They serve different purposes:

- **redb cache** (always on) — performance. If a file's SHA-256 hash hasn't changed, return the cached `Report` instantly without re-running any analyzers.
- **corpus store** (opt-in) — data collection. Every result is written to SQLite in two tables:
  - `corpus_entries` — one deduplicated row per unique file hash, recording its attribution and confidence.
  - `trend_entries` — a timestamped row on every analysis run (no deduplication). This lets you plot how a file's attribution drifts over time as you edit it or as the heuristics improve.

To enable the corpus store:

```bash
cargo add vibecheck-core --features corpus
```

### TUI Codebase Navigator

Interactive terminal UI — run `vibecheck` (no args) or `vibecheck tui <path>` to browse AI likelihood across an entire codebase as a two-pane file tree. Confidence scores roll up from symbol → file → directory (weighted by lines of code). The right pane shows score bars, every signal, and a per-symbol breakdown. The detail pane is scrollable with `d`/`u` when there are more signals than fit on screen.

### Historical & Live Trend Tracking

```bash
# Watch a directory live — re-analyze on save, print deltas
vibecheck watch src/

# Walk git history for a file and show attribution changes across commits
vibecheck history src/pipeline.rs --limit 20
```

`history` reads blobs directly from the git object store (no checkout needed). `watch` uses OS filesystem events with a 300 ms debounce and a 2 s per-file cooldown to suppress duplicate events from a single save.

## What's Coming

```
  THE GRAND PLAN (revised)
  ──────────────────────────────────────────────────────
  v0.1 - "It Works On My Machine"          ✓ shipped
  v0.2 - "Infrastructure That Doesn't Lie" ✓ shipped
         (Merkle cache, symbol-level, TUI,
          watch mode, git history)
  v0.3 - "Please Don't Scan My node_modules" ✓ shipped
         (ignore rules, .vibecheck config, IgnoreRules DI)
  v0.4 - "Trust No Signal You Can't Override"  ✓ shipped
         (heuristics config system, signal IDs, weight overrides,
          vibecheck heuristics command, TUI history panel)
  v0.5 - "Your Codebase Has a Trend Problem" <- next
         (persistent trend store, sparklines)
  v0.6 - "We Trained a Model On This"
  v1.0 - "Skynet But For Code Review"
  ──────────────────────────────────────────────────────
```

## Roadmap

### Phase 1 — Infrastructure ✅
- [x] **Crate split** — `vibecheck-core` (library) + `vibecheck-cli` (binary)
- [x] **Content-addressed cache** — SHA-256 per file; skip re-analysis of unchanged files (redb)
- [x] **tree-sitter CST analysis** — Rust (5 signals), Python (3 signals), JavaScript (3 signals), Go (3 signals)
- [x] **Corpus store** — SQLite-backed labeled dataset + trend log, feature-gated (`--features corpus`)
- [x] **Library API** — `vibecheck-core` is a clean library crate with no CLI dependencies
- [x] **JSON output** — pipe results to other tools
- [x] **GitHub Action** — run vibecheck in CI, fail PRs based on AI attribution (`--assert-family`)

### Phase 2 — Visible Product ✅
- [x] **Historical trend tracking** — `vibecheck history <path>` replays git log
- [x] **Live watch mode** — `vibecheck watch <path>` re-analyzes on file saves
- [x] **TUI navigator** — ratatui-based codebase browser with confidence bars
- [x] **Symbol-level attribution** — `vibecheck --symbols <file>` breaks down each function/method
- [x] **Merkle hash tree** — incremental directory analysis; unchanged subtrees are skipped entirely
- [x] **Ignore rules** — `.vibecheck` config file; auto-respects `.gitignore`; `--ignore-file` flag; `IgnoreRules` trait for DI in library consumers

### Phase 3 — Corpus Growth
- [ ] **Git repo scraper** — acquire labeled corpus from public repos via commit co-author metadata

### Phase 4 — Intelligence
- [ ] **ML classification** — `linfa`-based model trained on scraped corpus; replaces hand-tuned weights
- [ ] **Version detection** — distinguish Claude 3.5 vs Claude 4, GPT-3.5 vs GPT-4o (corpus permitting)
- [ ] **Plugin system** — WASM-based external analyzers
- [ ] **Benchmark suite** — accuracy metrics against known human/AI code datasets

### Already Shipped
- [x] **6 text-pattern analyzers** — comment style, AI signals, error handling, naming, code structure, idiom usage
- [x] **tree-sitter CST analyzers** — Rust (5), Python (3), JavaScript (3), Go (3)
- [x] **Content-addressed cache** — redb backend, SHA-256 keyed, instant on cache hit
- [x] **Merkle hash tree** — SHA-256 of sorted child hashes; unchanged directory subtrees are skipped entirely
- [x] **Symbol-level attribution** — per-function/method `SymbolReport` with its own `Attribution` + `Signal` list
- [x] **TUI navigator** — ratatui-based two-pane browser (file tree + detail panel)
- [x] **Live watch mode** — OS FS events (inotify/kqueue/FSEvents) with 300 ms debounce + 2 s per-file cooldown
- [x] **Git history replay** — reads blobs from the git object store, no working-tree checkout
- [x] **Corpus store** — accumulates labeled samples and per-file trend history in SQLite (`--features corpus`)
- [x] **GitHub Action** — run vibecheck in CI, fail PRs based on AI attribution (`--assert-family`)
- [x] **JSON output** — pipe results to other tools
- [x] **Library API** — `vibecheck-core` is a clean library crate with no CLI dependencies
- [x] **Ignore rules** — `.vibecheck` TOML config; gitignore-style patterns; `IgnoreRules` trait for DI; `--ignore-file` flag
- [x] **Heuristics system** — 151 signals with stable IDs; per-signal weight overrides in `.vibecheck`; `vibecheck heuristics` command; `HeuristicsProvider` DI trait; TUI git history panel (`h` to toggle)

## Limitations

```
  ┌─────────────────────────────────────────────────┐
  │                                                 │
  │  DISCLAIMER (legally required vibes disclosure) │
  │                                                 │
  │  vibecheck is a heuristic tool.                 │
  │  It detects VIBES, not PROOF.                   │
  │                                                 │
  │  A meticulous human might code like Claude.     │
  │  A sloppy prompt might produce messy AI.        │
  │                                                 │
  │  Do NOT use this to:                            │
  │    - accuse your coworker in a code review      │
  │    - settle bets on who wrote the bug           │
  │    - submit as evidence in a court of law       │
  │                                                 │
  │  DO use this to:                                │
  │    - win bets on who wrote the bug              │
  │    - roast your team's PR descriptions          │
  │    - feel seen when it detects your AI code     │
  │                                                 │
  │  (Also, this entire crate was written by an AI  │
  │   so we are absolutely not throwing stones.)    │
  │                                                 │
  └─────────────────────────────────────────────────┘
```

**Current limitations:**
- **Heuristic-based** — no ML model; weights are hand-tuned, not learned from a corpus
- **Not adversarial-resistant** — deliberately obfuscated AI code will fool it
- **Model family overlap** — GPT and Claude share many patterns; attribution between them is fuzzy
- **Symbol-level is file-cached** — `--symbols` results are cached per file hash; mixed authorship within a file is detected but symbol boundaries depend on tree-sitter parse quality
- **Watch/history are read-only** — no persistent trend store yet; trend deltas are printed to stdout only

## Contributing

Contributions welcome! Some high-impact areas:

1. **More signals** — if you notice a pattern that screams "AI wrote this", open a PR
2. **Weight tuning** — help calibrate signal weights against real-world code
3. **More CST signals** — extend the existing JS/Go/Rust/Python CST analyzers or add a new language (implement `CstAnalyzer` and register in `default_cst_analyzers()`)
4. **Test corpus** — curate labeled examples of human vs AI code for training and benchmarking
5. **New text analyzers** — implement the `Analyzer` trait (`analyze(&str) -> Vec<Signal>`) and register in `default_analyzers()`

## License

MIT

---

```
  Made with massive vibes by an AI that is fully aware
  of the irony of writing a tool to detect itself.

  ┌──────────────────────────────────────────────────┐
  │  $ vibecheck vibecheck-core                      │
  │                                                  │
  │  Verdict: Claude (81%)                           │
  │                                                  │
  │  Signals:                                        │
  │    [ai_signals] Zero TODOs, alphabetized         │
  │    imports, and every function has a doc         │
  │    comment. This is either a very disciplined    │
  │    human or — and I cannot stress this enough    │
  │    — a chatbot.                                  │
  │                                                  │
  │    Source: I am literally that chatbot.          │
  │                                                  │
  └──────────────────────────────────────────────────┘
```
