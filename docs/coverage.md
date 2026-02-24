# Line Coverage in VSCode

See inline coverage gutters (green/red line markers) for vibecheck while you edit.

## Prerequisites

```bash
cargo install cargo-llvm-cov
```

Install the [Coverage Gutters](https://marketplace.visualstudio.com/items?itemName=ryanluker.vscode-coverage-gutters)
VSCode extension (`ryanluker.vscode-coverage-gutters`).

## Generate coverage

```bash
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

Coverage Gutters auto-discovers `lcov.info` in the workspace root.
Click **Watch** in the VSCode status bar (or run **Coverage Gutters: Display Coverage**) to activate the gutters.

## CI threshold

The CI pipeline enforces ≥ 80% line coverage across the workspace:

```bash
cargo llvm-cov --workspace --fail-under-lines 80
```

## Tips

- Run `cargo llvm-cov --workspace --open` to view an HTML report in your browser.
- The `.cargo/config.toml` in this repo sets `LLVM_PROFILE_FILE` so profraw
  files land in `target/` and are discovered automatically — no extra flags needed.
- Add `lcov.info` to `.gitignore` if you don't want it committed (it is already ignored).
