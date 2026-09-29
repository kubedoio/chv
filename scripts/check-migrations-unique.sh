#!/usr/bin/env bash
#
# check-migrations-unique.sh — Enforce per-directory migration version
# uniqueness.
#
# Background: the three migrations directories are independent version
# namespaces:
#   - cmd/chv-controlplane/migrations/
#   - crates/cellhv-core-store/migrations/
#   - crates/chv-nwd-core/migrations/
# Each legitimately restarts numbering at 0001. A duplicate version number
# WITHIN one directory breaks every fresh database boot (sqlx's migrations
# table has version as its PRIMARY KEY — run 8a failed with
# "UNIQUE constraint failed: _sqlx_migrations.version"), which is what the
# renumber in PR #289 hit. Merge-ref CI
# cannot catch these collisions: each PR's diff only shows its own files,
# so two PRs can each add e.g. 0056_*.sql and only collide after merge.
#
# This gate fails when:
#   - two migration .sql files in the same directory share the same
#     leading numeric `<version>_` prefix
#   - a migration .sql filename has no numeric prefix at all
#
# Uniqueness is enforced PER-DIRECTORY only; cross-directory reuse of a
# version number is fine (the three namespaces each restart at 0001).
#
# Exit codes:
#   0 — no violations
#   1 — violations found (printed to stderr)
#   2 — script invocation error

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Discover tracked migration .sql files; fall back to find when not in a
# git checkout (e.g., a source tarball).
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  mapfile -t SQL_FILES < <(git ls-files -- '*.sql')
else
  mapfile -t SQL_FILES < <(find . -type f -name '*.sql' \
    -not -path './target/*' \
    -not -path '*/node_modules/*' \
    | sed 's|^\./||' | sort)
fi

VIOLATIONS=0

# Key: "<dir><TAB><version>" -> first file that claimed it.
declare -A SEEN

for file in "${SQL_FILES[@]}"; do
  dir="$(dirname "$file")"
  base="$(basename "$file")"

  if [[ ! "$base" =~ ^([0-9]+)_ ]]; then
    echo "Migration uniqueness violation: '$file' has no numeric prefix." >&2
    echo "Migration files must be named <version>_<description>.sql (e.g. 0001_initial.sql)." >&2
    VIOLATIONS=1
    continue
  fi

  version="${BASH_REMATCH[1]}"
  key="${dir}"$'\t'"${version}"

  if [[ -n "${SEEN[$key]:-}" ]]; then
    echo "Migration uniqueness violation: duplicate version ${version} in ${dir}/:" >&2
    echo "  ${SEEN[$key]}" >&2
    echo "  ${file}" >&2
    VIOLATIONS=1
  else
    SEEN["$key"]="$file"
  fi
done

if [[ "$VIOLATIONS" -ne 0 ]]; then
  echo "" >&2
  echo "Fix: renumber the offending migration so each version number is unique" >&2
  echo "within its directory. Version namespaces are per-directory (each of" >&2
  echo "cmd/chv-controlplane/migrations/, crates/cellhv-core-store/migrations/," >&2
  echo "and crates/chv-nwd-core/migrations/ restarts at 0001)." >&2
  exit 1
fi

echo "Migration uniqueness check passed: no duplicate versions or missing prefixes."
exit 0
