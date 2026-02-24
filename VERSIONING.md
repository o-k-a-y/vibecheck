# Versioning Policy

vibecheck follows [Semantic Versioning](https://semver.org) (`MAJOR.MINOR.PATCH`).
Both `vibecheck-core` and `vibecheck-cli` share a single version via the
Cargo workspace — one change in `Cargo.toml` updates both crates simultaneously.

---

## When to bump

### Patch — `0.x.PATCH`

Bug fixes and improvements with no externally-visible API or behaviour change.

| Trigger | Example |
|---------|---------|
| Bug fix in existing logic | Fix incorrect confidence score roll-up |
| Performance improvement | Faster Merkle tree walk |
| Internal refactor (no public API change) | Rename a private helper |
| Dependency patch-level update | `ignore` 0.4.x → 0.4.y |
| Documentation or README typo fix | Correct a flag description |
| Flaky test fix | Raise timing threshold in `single_file_analysis_under_100ms` |

### Minor — `0.MINOR.0`

New functionality that is **backwards-compatible** — existing code continues
to compile and behave identically.

| Trigger | Example |
|---------|---------|
| New public function or type in `vibecheck-core` | `analyze_directory_with`, `IgnoreRules` trait |
| New CLI flag or subcommand | `--ignore-file`, `vibecheck watch` |
| New config file option | New key in `.vibecheck` `[ignore]` section |
| New language support in CST analyzer | Adding TypeScript grammar |
| New analyzer (text or CST) | New `Analyzer` impl registered in `default_analyzers()` |
| New feature-gated capability | New `--features` flag in `vibecheck-core` |

### Major — `MAJOR.0.0`

**Breaking changes** — existing callers must update their code.

| Trigger | Example |
|---------|---------|
| Renamed or removed public function | Rename `analyze_directory` → `scan_directory` |
| Changed public function signature | Add required param to `analyze_file` |
| Renamed or removed field in `Report`, `Signal`, `Attribution`, `SymbolReport` | Remove `Report.metadata.signal_count` |
| Renamed or removed CLI flag | Rename `--no-cache` → `--skip-cache` |
| Removed subcommand | Drop `vibecheck history` |
| Changed cache format or storage layout | Incompatible redb schema change |

---

## How to bump

Use the bump script — it updates both version fields in the root `Cargo.toml`
and prints a checklist:

```bash
./scripts/bump-version.sh patch   # 0.3.0 → 0.3.1
./scripts/bump-version.sh minor   # 0.3.0 → 0.4.0
./scripts/bump-version.sh major   # 0.3.0 → 1.0.0
```

The script will:
1. Read the current version from `[workspace.package]`
2. Compute the new version
3. Update both occurrences in `Cargo.toml` (`[workspace.package]` and the
   `vibecheck-core` workspace dependency)
4. Verify the build still compiles
5. Print the commit command to use

### After running the script

```bash
# The script confirms the build — then commit:
git add Cargo.toml Cargo.lock
git commit -m "chore: bump to vX.Y.Z"
```

---

## Checklist before publishing to crates.io

- [ ] `./scripts/bump-version.sh [level]` ran cleanly and build passes
- [ ] `cargo test --workspace` is green
- [ ] CHANGELOG / commit messages reflect what changed since last release
- [ ] README "What's Coming" roadmap updated if a milestone was reached
- [ ] `git tag vX.Y.Z` created on the release commit
- [ ] `cargo publish -p vibecheck-core` (publish core first — cli depends on it)
- [ ] Wait for crates.io to index, then `cargo publish -p vibecheck-cli`

---

## Version bump decision guide

```
Is this a breaking change to the public API or CLI flags?
  YES → major

Is this new functionality (new API, new flag, new config option)?
  YES → minor

Is this a bug fix or internal change with no visible effect on callers?
  YES → patch
```

When in doubt: if a downstream library consumer (e.g. `ambits`) would need
to change their code or config after upgrading, it is at minimum a minor bump.
If they cannot upgrade without changing code that previously worked, it is major.
