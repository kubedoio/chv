#!/usr/bin/env bash
# apply-branch-protection.sh
#
# Apply branch protection rules to the CHV repository using the GitHub CLI.
# Must be run by a repository admin with push access to the default branch.
#
# Usage:
#   ./scripts/github-setup/apply-branch-protection.sh
#
# Requirements:
#   - gh CLI installed and authenticated (gh auth status)
#   - REPO_OWNER and REPO_NAME inferred from git remote origin
#
# The protection rule applied here matches the specification in:
#   docs/governance/BRANCH_PROTECTION.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# ---------------------------------------------------------------------------
# Resolve repo slug from git remote
# ---------------------------------------------------------------------------
REMOTE_URL=$(cd "${REPO_ROOT}" && git remote get-url origin 2>/dev/null || true)
REPO_SLUG=""

if [ -n "${REMOTE_URL}" ]; then
    # Handle both HTTPS and SSH remotes
    if [[ "${REMOTE_URL}" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
        REPO_SLUG="${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    fi
fi

if [ -z "${REPO_SLUG}" ]; then
    echo "ERROR: Could not determine repository slug from git remote."
    echo "       Set REPO_SLUG manually, e.g.:"
    echo "       REPO_SLUG=kubedoio/chv ./scripts/github-setup/apply-branch-protection.sh"
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
# Apply branch protection to 'main' via GitHub API
# ---------------------------------------------------------------------------
echo "Applying branch protection rules to 'main' ..."

# Build the JSON payload. We use the REST API directly because the gh CLI
# 'gh api' command is more predictable than 'gh repo edit' for protection.
gh api --method PUT "repos/${REPO_SLUG}/branches/main/protection" \
    --header "Accept: application/vnd.github+json" \
    --input - <<'EOF'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Rust checks",
      "UI checks"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "required_approving_review_count": 1,
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": true,
    "require_last_push_approval": false,
    "required_review_thread_resolution": true
  },
  "restrictions": null,
  "required_signatures": true,
  "required_linear_history": false,
  "allow_force_pushes": false,
  "allow_deletions": false
}
EOF

echo "Branch protection applied successfully to 'main'."
echo ""
echo "Next steps:"
echo "  1. Verify in GitHub UI: Settings → Branches → main"
echo "  2. Ensure GitHub teams referenced in .github/CODEOWNERS exist and have write access."
echo "  3. Run ./scripts/github-setup/verify-settings.sh to confirm."
