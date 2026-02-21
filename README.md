# vibecheck

```
   ┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐┌─┐
   │v││i││b││e││c││h││e││c││k│
   └─┘└─┘└─┘└─┘└─┘└─┘└─┘└─┘└─┘
   sniff out the AI slop 🔍🤖
```

```
   ┌──────────────────────────────────────────┐
   │                                          │
   │   👁️  I can smell your AI slop.          │
   │                                          │
   │   Your code is organized.                │
   │   Too organized.                         │
   │   ...Suspiciously organized.             │
   │                                          │
   │   Verdict: Claude (81% confidence)       │
   │                                          │
   │          ┌───────────┐                   │
   │   Claude │███████████│ 81%               │
   │   GPT    │████       │ 19%               │
   │   Human  │           │  0%  ← yeah right │
   │          └───────────┘                   │
   │                                          │
   └──────────────────────────────────────────┘
```

> *"I don't always write Rust, but when I do, every function has a doc comment and zero `.unwrap()` calls."*
> *— The Most Interesting LLM in the World*

**vibecheck** is a Rust crate that detects AI-generated code and attributes it to a model family. It sniffs out the telltale "vibes" that different AI models leave in code — the suspiciously perfect formatting, the teaching-voice comments, the conspicuous absence of `TODO: fix this later`.

```
   No TODOs?  No dead code?  Every function documented?

            ╔══════════════════════════╗
            ║                          ║
            ║   That's not a developer ║
            ║   That's a chatbot       ║
            ║                          ║
            ╚════════════╤═════════════╝
                         │
                    ┌────┴────┐
                    │  ⊙    ⊙ │
                    │    ◡    │
                    │ ┌─────┐ │
                    │ │ 100 │ │  < certified AI slop score
                    │ └─────┘ │
                    └─────────┘

   "I reviewed your PR. Every variable is named
    'descriptive_and_meaningful_context_value'.
    Nobody writes code like that, Dave."
```

## How It Works

vibecheck runs your source code through **6 heuristic analyzers**, each looking for different "tells":

| Analyzer | What It Sniffs | Example Signal |
|----------|---------------|----------------|
| **Comment Style** | Density, teaching voice, doc comments | *"12 comments with teaching/explanatory voice"* |
| **AI Signals** | TODO absence, no dead code, eerie perfection | *"Every function has a doc comment — suspiciously thorough"* |
| **Error Handling** | unwrap vs expect vs ?, panic usage | *"Zero .unwrap() calls — careful error handling"* |
| **Naming** | Variable length, descriptiveness, single-char names | *"Very descriptive variable names (avg 14.2 chars)"* |
| **Code Structure** | Type annotations, import ordering, formatting | *"Import statements are alphabetically sorted"* |
| **Idiom Usage** | Iterator chains, builder patterns, Display impls | *"8 iterator chain usages — textbook-idiomatic Rust"* |

Each signal has a **weight** (positive = evidence for, negative = evidence against) and points to a **model family**. The pipeline aggregates all signals into a probability distribution.

```
 ┌────────────────────────┬────────────────────────┬────────────────────────┐
 │   THE AI CODE          │   ALIGNMENT            │   CHART                │
 ├────────────────────────┼────────────────────────┼────────────────────────┤
 │                        │                        │                        │
 │  CLAUDE                │  GPT                   │  COPILOT               │
 │                        │                        │                        │
 │  /// Every function    │  let x: i32 = 5;       │  fn main() {           │
 │  /// is documented.    │  // types on           │    things().unwrap();  │
 │  pub fn perfectly_     │  // EVERYTHING         │    stuff().unwrap();   │
 │  named_function()      │  impl Builder {        │    more().unwrap();    │
 │                        │    fn with_x()         │    // works lol        │
 │  Zero .unwrap() calls  │    fn with_y()         │  }                     │
 │  Sorted imports        │    fn with_z()         │                        │
 │  format!() only        │    fn build()          │  "ship it"             │
 │                        │                        │                        │
 ├────────────────────────┼────────────────────────┼────────────────────────┤
 │                        │                        │                        │
 │  GEMINI                │  HUMAN                 │  HUMAN (at 2 AM)       │
 │                        │                        │                        │
 │  (we're still          │  // TODO               │  // WHY DOES THIS WORK │
 │   collecting           │  // HACK               │  // DO NOT TOUCH       │
 │   data on this one)    │  // FIXME later        │  let x = 42;           │
 │                        │  let x = 42;           │  let xx = x;           │
 │                        │  let mut s = "";       │  // let xxx = xx;      │
 │  🔬                    │  s = s + &thing;       │  panic!("WHY");        │
 │                        │                        │                        │
 └────────────────────────┴────────────────────────┴────────────────────────┘
```

## Installation

```bash
# Clone and build
git clone https://github.com/youruser/vibecheck.git
cd vibecheck
cargo build --release

# Or add as a library dependency (without CLI deps)
# Cargo.toml:
# vibecheck = { path = ".", default-features = false }
```

## Usage

### CLI

```bash
# Analyze a single file (pretty output with colors)
vibecheck src/main.rs

# Analyze a directory
vibecheck src/

# Plain text output
vibecheck src/lib.rs --format text

# JSON output (for piping to other tools)
vibecheck src/ --format json
```

### Example Output

```
$ vibecheck src/pipeline.rs

File: src/pipeline.rs
Verdict: Claude (72% confidence)
Lines: 86 | Signals: 12

Scores:
  Claude     █████████████████████ 72.5%
  GPT        ██████ 22.9%
  Copilot    █ 4.6%
  Gemini      0.0%
  Human       0.0%

Signals:
  [ai_signals] +1.5 Claude — No TODO/FIXME markers in a substantial file
  [ai_signals] +0.8 Claude — No dead code suppressions
  [ai_signals] +0.5 GPT   — Zero trailing whitespace — machine-perfect formatting
  [errors]     +0.5 Copilot — 2 .unwrap() calls — moderate
  [naming]     +1.0 Claude — No single-character variable names
  [idioms]     +1.5 Claude — 6 iterator chain usages — textbook-idiomatic Rust
  [idioms]     +1.0 GPT   — 11 method chain continuation lines — builder pattern
  ...
```

### The Ultimate Test: Self-Detection

vibecheck was written by an AI. Does it know?

```
$ vibecheck src/analyzers/comment_style.rs --format text

Verdict: Claude (81% confidence)      # 👀 it knows

$ vibecheck tests/self_detection.rs --format text

Verdict: Human (46% confidence)       # test code is messier, more "human"
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
use vibecheck::report::ModelFamily;

// Analyze a string
let report = vibecheck::analyze(source_code);
println!("Verdict: {} ({:.0}%)",
    report.attribution.primary,
    report.attribution.confidence * 100.0);

// Analyze a file
let report = vibecheck::analyze_file(Path::new("suspect.rs"))?;
if report.attribution.primary != ModelFamily::Human {
    println!("Caught one! This code was probably written by {}",
        report.attribution.primary);
}
```

## Architecture

```
                    ┌──────────┐
   source code ───> │ Pipeline │
                    └────┬─────┘
                         │
          ┌──────────────┼──────────────┐
          │              │              │
    ┌─────┴─────┐  ┌────┴────┐    ┌─────┴─────┐
    │ Comment   │  │ AI      │    │ Error     │  ... (6 total)
    │ Style     │  │ Signals │    │ Handling  │
    └─────┬─────┘  └────┬────┘    └─────┬─────┘
          │              │              │
          └──────── Signals ────────────┘
                         │
                  ┌──────┴──────┐
                  │  Aggregate  │
                  │  Normalize  │
                  │  Attribute  │
                  └──────┬──────┘
                         │
                      Report
              (family + confidence + signals)
```

## Model Family Profiles

How vibecheck tells them apart:

- **Claude**: Thorough doc comments, teaching voice, zero `unwrap()`, textbook iterator chains, `format!()` over concatenation, sorted imports, suspiciously complete
- **GPT**: Explicit type annotations, builder patterns, method chaining, explanatory (but less pedagogical) comments
- **Copilot**: Works but cuts corners — moderate `unwrap()` usage, less documentation, pragmatic completion style
- **Gemini**: Currently limited signal set (future improvement area)
- **Human**: TODOs everywhere, `// HACK`, commented-out code, single-character variables, `panic!()` calls, string concatenation, chaotic formatting

## Feature Flags

| Feature | Default | What it enables |
|---------|---------|-----------------|
| `cli` | Yes | `clap`, `walkdir`, `colored`, `anyhow` for the CLI binary |

To use vibecheck as a library without CLI dependencies:

```toml
[dependencies]
vibecheck = { version = "0.1", default-features = false }
```

## Roadmap

```
  THE GRAND PLAN
  ────────────────────────────────────────
  v0.1 - "It Works On My Machine"      ← you are here
  v0.2 - "Getting Smarter"
  v0.3 - "Polyglot"
  v0.4 - "The Integrations"
  v1.0 - "Production Vibes"
  ────────────────────────────────────────
```

### v0.2 — Getting Smarter
- [ ] **Weighted signal tuning** — calibrate weights against a labeled corpus of human/AI code
- [ ] **Gemini-specific signals** — better differentiation for Gemini-generated code
- [ ] **Confidence calibration** — ensure reported confidence matches actual accuracy
- [ ] **Combined file analysis** — aggregate signals across an entire crate for a project-level verdict
- [ ] **Configurable thresholds** — let users tune sensitivity

### v0.3 — Polyglot
- [ ] **Python support** — detect AI patterns in Python (docstring style, type hints, f-strings)
- [ ] **TypeScript/JavaScript support** — JSDoc patterns, import styles, async patterns
- [ ] **Go support** — error handling patterns, naming conventions, comment style
- [ ] **Language auto-detection** — pick the right analyzer set automatically

### v0.4 — The Integrations
- [ ] **GitHub Action** — run vibecheck in CI, annotate PRs with AI attribution
- [ ] **Pre-commit hook** — flag AI-generated code before it lands
- [ ] **Editor plugins** — VS Code extension showing inline AI probability
- [ ] **Git blame integration** — attribute commits, not just files

### v1.0 — Production Vibes
- [ ] **ML-backed scoring** — train a classifier on the heuristic signals for better accuracy
- [ ] **AST-aware analysis** — parse actual syntax trees instead of string matching
- [ ] **Regex patterns** — more sophisticated pattern matching for v1 heuristics
- [ ] **Benchmark suite** — accuracy metrics against known human/AI code datasets
- [ ] **Watermark detection** — detect known AI watermarking patterns

## Limitations

```
  ┌─────────────────────────────────────────────────┐
  │                                                 │
  │  DISCLAIMER                                     │
  │                                                 │
  │  vibecheck is a heuristic tool.                 │
  │  It detects VIBES, not PROOF.                   │
  │                                                 │
  │  A meticulous human might code like Claude.     │
  │  A sloppy prompt might produce messy AI.        │
  │                                                 │
  │  Use for fun and insight, not for               │
  │  high-stakes attribution decisions.             │
  │                                                 │
  │  (Also, this entire crate was written by        │
  │   an AI, so take that as you will.)             │
  │                                                 │
  └─────────────────────────────────────────────────┘
```

- **Rust-only** (for now) — other languages coming in v0.3
- **Heuristic-based** — no ML, no AST parsing, just string vibes
- **Not adversarial-resistant** — deliberately obfuscated AI code will fool it
- **Model family overlap** — GPT and Claude share many patterns; attribution between them is fuzzy
- **File-level only** — can't detect mixed human/AI authorship within a single file

## Contributing

Contributions welcome! Some high-impact areas:

1. **More signals** — if you notice a pattern that screams "AI wrote this", open a PR
2. **Weight tuning** — help calibrate signal weights against real-world code
3. **Language support** — add analyzers for Python, TypeScript, Go, etc.
4. **Test corpus** — curate labeled examples of human vs AI code

## License

MIT

---

```
  Made with massive vibes by an AI that is fully aware
  of the irony of writing a tool to detect itself.

  🤖 ──> 🔍 ──> 🤖
       "It me."

  ┌──────────────────────────────────────────┐
  │  vibecheck src/lib.rs                    │
  │  > Verdict: Claude (78%)                 │
  │                                          │
  │  vibecheck src/README.md                 │
  │  > error: no .rs files found             │
  │  > (nice try though)                     │
  └──────────────────────────────────────────┘
```
