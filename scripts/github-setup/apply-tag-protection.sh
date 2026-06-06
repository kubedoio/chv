#!/usr/bin/env bash
# apply-tag-protection.sh
#
# Apply tag protection rules to the CHV repository using the GitHub CLI.
# Must be run by a repository admin.
#
# Usage:
#   ./scripts/github-setup/apply-tag-protection.sh
#
# This prevents non-admin/maintainer roles from creating version tags that
# could trigger the release workflow.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# ---------------------------------------------------------------------------
# Resolve repo slug from git remote
# ---------------------------------------------------------------------------
REMOTE_URL=$(cd "${REPO_ROOT}" && git remote get-url origin 2>/dev/null || true)
REPO_SLUG=""

if [ -n "${REMOTE_URL}" ]; then
    if [[ "${REMOTE_URL}" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
        REPO_SLUG="${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    fi
fi

if [ -z "${REPO_SLUG}" ]; then
    echo "ERROR: Could not determine repository slug from git remote."
    echo "       Set REPO_SLUG manually, e.g.:"
    echo "       REPO_SLUG=kubedoio/chv ./scripts/github-setup/apply-tag-protection.sh"
    exit 1
fi

echo "Target repository: ${REPO_SLUG}"

# ---------------------------------------------------------------------------
# Verify gh CLI
# ---------------------------------------------------------------------------
if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: gh CLI is not installed. Install from https://cli.github.com/"
    exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
    echo "ERROR: gh CLI is not authenticated. Run: gh auth login"
    exit 1
fi

# ---------------------------------------------------------------------------
# Apply tag protection via GitHub API
# ---------------------------------------------------------------------------
echo "Applying tag protection rule for pattern 'v*' ..."

# GitHub tag protection API: POST /repos/{owner}/{repo}/tags/protection
# Note: This is the legacy tag protection endpoint. For Rulesets (preferred),
# use the rulesets API instead (see docs/governance/BRANCH_PROTECTION.md).
gh api --method POST "repos/${REPO_SLUG}/tags/protection" \
    --header "Accept: application/vnd.github+json" \
    --field "pattern=v*"

echo "Tag protection applied successfully for pattern 'v*'."
echo ""
echo "Next steps:"
echo "  1. Verify in GitHub UI: Settings → Tags"
echo "  2. Consider migrating to Tag Rulesets for finer-grained control."
