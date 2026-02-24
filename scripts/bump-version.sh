#!/usr/bin/env bash
# bump-version.sh — bump the shared workspace version for vibecheck-core and vibecheck-cli
#
# Usage:
#   ./scripts/bump-version.sh patch   # 0.3.0 → 0.3.1
#   ./scripts/bump-version.sh minor   # 0.3.0 → 0.4.0
#   ./scripts/bump-version.sh major   # 0.3.0 → 1.0.0
#
# See VERSIONING.md for the policy on when to use each level.

set -euo pipefail

LEVEL="${1:-}"
ROOT_TOML="$(git rev-parse --show-toplevel)/Cargo.toml"

# ---------------------------------------------------------------------------
# Validate arguments
# ---------------------------------------------------------------------------

if [[ -z "$LEVEL" || ! "$LEVEL" =~ ^(major|minor|patch)$ ]]; then
    echo "Usage: $0 [major|minor|patch]" >&2
    echo "See VERSIONING.md for the policy on when to use each level." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Read current version from [workspace.package]
# ---------------------------------------------------------------------------

CURRENT=$(grep -m1 '^version = ' "$ROOT_TOML" | sed 's/version = "\(.*\)"/\1/')

if [[ -z "$CURRENT" ]]; then
    echo "error: could not find 'version = \"...\"' in $ROOT_TOML" >&2
    exit 1
fi

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

# ---------------------------------------------------------------------------
# Compute new version
# ---------------------------------------------------------------------------

case "$LEVEL" in
    major) MAJOR=$((MAJOR + 1)); MINOR=0; PATCH=0 ;;
    minor) MINOR=$((MINOR + 1)); PATCH=0 ;;
    patch) PATCH=$((PATCH + 1)) ;;
esac

NEW="$MAJOR.$MINOR.$PATCH"

echo "Bumping $CURRENT → $NEW ($LEVEL)"

# ---------------------------------------------------------------------------
# Update Cargo.toml (both occurrences: [workspace.package] and the
# vibecheck-core workspace dependency entry)
# ---------------------------------------------------------------------------

# Portable sed: use a temp file to avoid -i incompatibilities on macOS/Linux.
TMP=$(mktemp)
sed "s/version = \"$CURRENT\"/version = \"$NEW\"/g" "$ROOT_TOML" > "$TMP"
mv "$TMP" "$ROOT_TOML"

# Sanity check: ensure new version appears and old one is gone
if grep -q "version = \"$CURRENT\"" "$ROOT_TOML"; then
    echo "error: old version '$CURRENT' still present in $ROOT_TOML after replacement" >&2
    exit 1
fi

if ! grep -q "version = \"$NEW\"" "$ROOT_TOML"; then
    echo "error: new version '$NEW' not found in $ROOT_TOML after replacement" >&2
    exit 1
fi

echo "Updated $ROOT_TOML"

# ---------------------------------------------------------------------------
# Verify the build still compiles (updates Cargo.lock as a side-effect)
# ---------------------------------------------------------------------------

echo "Verifying build…"
cargo build --workspace --quiet
echo "Build OK"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo ""
echo "────────────────────────────────────────"
echo "  vibecheck-core  $CURRENT → $NEW"
echo "  vibecheck-cli   $CURRENT → $NEW"
echo "────────────────────────────────────────"
echo ""
echo "Next steps:"
echo "  git add Cargo.toml Cargo.lock"
echo "  git commit -m \"chore: bump to v$NEW\""
echo ""
echo "Before publishing:"
echo "  cargo test --workspace"
echo "  cargo publish -p vibecheck-core"
echo "  cargo publish -p vibecheck-cli"
