#!/bin/bash
# Bump the project version across all relevant files.
# Usage: ./scripts/bump-version.sh [major|minor|patch|build]
# Default bump type is "build".
# Pass --dry-run as a second argument to preview changes without writing files.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

BUMP_TYPE="${1:-build}"
DRY_RUN="${2:-}"

# ---------------------------------------------------------------------------
# Validate bump type
# ---------------------------------------------------------------------------
case "$BUMP_TYPE" in
  major|minor|patch|build)
    ;;
  *)
    echo "Unknown bump type: $BUMP_TYPE" >&2
    echo "Usage: $0 [major|minor|patch|build] [--dry-run]" >&2
    exit 1
    ;;
esac

# ---------------------------------------------------------------------------
# Read current version
# ---------------------------------------------------------------------------
OLD_VERSION=$(cat VERSION)
IFS='.' read -r MAJOR MINOR PATCH BUILD <<< "$OLD_VERSION"

# ---------------------------------------------------------------------------
# Compute new version
# ---------------------------------------------------------------------------
case "$BUMP_TYPE" in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    BUILD=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    BUILD=0
    ;;
  patch)
    PATCH=$((PATCH + 1))
    BUILD=0
    ;;
  build)
    BUILD=$((BUILD + 1))
    ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}.${BUILD}"

if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "[DRY RUN] Would bump: ${OLD_VERSION} -> ${NEW_VERSION}"
  exit 0
fi

echo "Bumping version: ${OLD_VERSION} -> ${NEW_VERSION}"

# ---------------------------------------------------------------------------
# 1. VERSION (source of truth)
# ---------------------------------------------------------------------------
echo "$NEW_VERSION" > VERSION

# ---------------------------------------------------------------------------
# 2. Cargo.toml workspace version
# ---------------------------------------------------------------------------
sed -i "s/^version = \"${OLD_VERSION}\"/version = \"${NEW_VERSION}\"/" Cargo.toml

# ---------------------------------------------------------------------------
# 3. UI package.json + package-lock.json (via npm, with sed fallback)
# ---------------------------------------------------------------------------
UI_OLD_VERSION=$(grep -m1 '"version"' ui/package.json | sed -E 's/.*"version": "([^"]+)".*/\1/')
if command -v npm &>/dev/null; then
  (
    cd ui
    npm --no-git-tag-version version "$NEW_VERSION" >/dev/null 2>&1 || true
  )
else
  sed -i "s/\"version\": \"${UI_OLD_VERSION}\"/\"version\": \"${NEW_VERSION}\"/" ui/package.json
  sed -i "s/\"version\": \"${UI_OLD_VERSION}\"/\"version\": \"${NEW_VERSION}\"/" ui/package-lock.json
fi

# ---------------------------------------------------------------------------
# 4. UI sidebar version label
# ---------------------------------------------------------------------------
SIDEBAR_FILE="ui/src/lib/components/shell/Sidebar.svelte"
if [ -f "$SIDEBAR_FILE" ]; then
  SIDEBAR_OLD_VERSION=$(grep -o 'chv-v[0-9][0-9.]*-alpha' "$SIDEBAR_FILE" | sed 's/chv-v//;s/-alpha//' || echo "$OLD_VERSION")
  sed -i "s/chv-v${SIDEBAR_OLD_VERSION}-alpha/chv-v${NEW_VERSION}-alpha/" "$SIDEBAR_FILE"
fi

# ---------------------------------------------------------------------------
# 5. Documentation & install scripts
# ---------------------------------------------------------------------------
sed -i "s/${OLD_VERSION}/${NEW_VERSION}/g" README.md
sed -i "s/${OLD_VERSION}/${NEW_VERSION}/g" docs/DEPLOYMENT.md
sed -i "s/${OLD_VERSION}/${NEW_VERSION}/g" scripts/install.sh
sed -i "s/${OLD_VERSION}/${NEW_VERSION}/g" scripts/hosting/cloudflare-worker.js
sed -i "s/${OLD_VERSION}/${NEW_VERSION}/g" scripts/hosting/github-pages-index.html

# ---------------------------------------------------------------------------
# 6. Update Cargo.lock so it stays in sync with Cargo.toml
# ---------------------------------------------------------------------------
if command -v cargo &>/dev/null; then
  cargo update --workspace >/dev/null 2>&1 || true
fi

echo "Version bumped to ${NEW_VERSION}"
echo ""
echo "Next steps:"
echo "  1. Review the diff: git diff"
echo "  2. Update CHANGELOG.md if this is a new release"
echo "  3. Commit the changes and optionally tag:"
echo "     git add -A && git commit -m \"release: bump version to ${NEW_VERSION}\""
echo "     git tag v${NEW_VERSION}"
