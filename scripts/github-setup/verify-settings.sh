#!/usr/bin/env bash
# verify-settings.sh
#
# Verify that the CHV repository hardening settings are in place.
# Prints a human-readable report; exits non-zero if critical settings are missing.
#
# Usage:
#   ./scripts/github-setup/verify-settings.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

ERRORS=0
WARNINGS=0

warn()  { echo "  [WARN]  $1"; ((WARNINGS++)); }
error() { echo "  [ERROR] $1"; ((ERRORS++)); }
ok()    { echo "  [OK]    $1"; }

# ---------------------------------------------------------------------------
# Resolve repo slug
# ---------------------------------------------------------------------------
REMOTE_URL=$(cd "${REPO_ROOT}" && git remote get-url origin 2>/dev/null || true)
REPO_SLUG=""

if [ -n "${REMOTE_URL}" ]; then
    if [[ "${REMOTE_URL}" =~ github\.com[:/]([^/]+)/([^/]+)(\.git)?$ ]]; then
        REPO_SLUG="${BASH_REMATCH[1]}/${BASH_REMATCH[2]}"
    fi
fi

if [ -z "${REPO_SLUG}" ]; then
    error "Could not determine repository slug from git remote."
    REPO_SLUG="${REPO_SLUG:-kubedoio/chv}"
fi

echo "========================================"
echo "CHV Repository Hardening Verification"
echo "Repository: ${REPO_SLUG}"
echo "========================================"
echo ""

# ---------------------------------------------------------------------------
# gh CLI availability
# ---------------------------------------------------------------------------
if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    GH_AVAILABLE=1
else
    GH_AVAILABLE=0
    warn "gh CLI not available or not authenticated; skipping live GitHub checks."
fi

# ---------------------------------------------------------------------------
# 1. CODEOWNERS file
# ---------------------------------------------------------------------------
echo "--- 1. CODEOWNERS ---"
if [ -f "${REPO_ROOT}/.github/CODEOWNERS" ]; then
    ok ".github/CODEOWNERS exists"
    TEAM_COUNT=$(grep -cE '@kubedoio/' "${REPO_ROOT}/.github/CODEOWNERS" || true)
    ok "CODEOWNERS references ${TEAM_COUNT} team entries"
else
    error ".github/CODEOWNERS is missing"
fi
echo ""

# ---------------------------------------------------------------------------
# 2. Workflow permissions
# ---------------------------------------------------------------------------
echo "--- 2. CI Workflow Permissions ---"
for workflow in ci.yml proto.yml security.yml package-pr.yml package-nightly.yml integration-kvm.yml release.yml; do
    wf_path="${REPO_ROOT}/.github/workflows/${workflow}"
    if [ -f "${wf_path}" ]; then
        if grep -qE '^permissions:' "${wf_path}"; then
            ok "${workflow} has explicit permissions block"
        else
            error "${workflow} is missing explicit permissions block"
        fi
    else
        warn "${workflow} not found"
    fi
done
echo ""

# ---------------------------------------------------------------------------
# 3. Live GitHub checks (requires gh CLI)
# ---------------------------------------------------------------------------
if [ "${GH_AVAILABLE}" -eq 1 ]; then
    echo "--- 3. Live GitHub Settings ---"

    # Branch protection for main
    BP_DATA=$(gh api "repos/${REPO_SLUG}/branches/main/protection" --header "Accept: application/vnd.github+json" 2>/dev/null || true)
    if [ -n "${BP_DATA}" ] && [ "${BP_DATA}" != "{" ]; then
        ok "Branch protection exists for 'main'"

        if echo "${BP_DATA}" | grep -q '"enforce_admins":{.*"enabled":true'; then
            ok "Admin enforcement is enabled"
        else
            warn "Admin enforcement may not be enabled"
        fi

        if echo "${BP_DATA}" | grep -q '"required_pull_request_reviews"'; then
            ok "PR reviews are required"
        else
            error "PR reviews are NOT required"
        fi

        if echo "${BP_DATA}" | grep -q '"required_status_checks"'; then
            ok "Status checks are required"
        else
            warn "Status checks may not be required"
        fi
    else
        error "No branch protection found for 'main'"
    fi

    # Tag protection
    TP_DATA=$(gh api "repos/${REPO_SLUG}/tags/protection" --header "Accept: application/vnd.github+json" 2>/dev/null || true)
    if [ -n "${TP_DATA}" ] && echo "${TP_DATA}" | grep -q '"pattern"'; then
        ok "Tag protection rule(s) exist"
    else
        warn "No tag protection rules found (legacy API); verify in UI or use Rulesets."
    fi

    # Default workflow permissions
    REPO_DATA=$(gh api "repos/${REPO_SLUG}" --header "Accept: application/vnd.github+json" 2>/dev/null || true)
    if echo "${REPO_DATA}" | grep -q '"default_workflow_permissions":"read"'; then
        ok "Default workflow token permissions are read-only"
    else
        warn "Default workflow token permissions may be write-all; recommended: read-only"
    fi
else
    echo "--- 3. Live GitHub Settings ---"
    warn "Skipped (gh CLI not available)"
fi
echo ""

# ---------------------------------------------------------------------------
# 4. Dependabot
# ---------------------------------------------------------------------------
echo "--- 4. Dependabot Configuration ---"
if [ -f "${REPO_ROOT}/.github/dependabot.yml" ]; then
    ok ".github/dependabot.yml exists"
    ECOSYSTEMS=$(grep -cE 'package-ecosystem:' "${REPO_ROOT}/.github/dependabot.yml" || true)
    ok "Dependabot configured for ${ECOSYSTEMS} ecosystem(s)"
else
    error ".github/dependabot.yml is missing"
fi
echo ""

# ---------------------------------------------------------------------------
# 5. Security policy
# ---------------------------------------------------------------------------
echo "--- 5. Security Policy ---"
if [ -f "${REPO_ROOT}/SECURITY.md" ]; then
    ok "SECURITY.md exists"
else
    warn "SECURITY.md is missing"
fi
echo ""

# ---------------------------------------------------------------------------
# 6. Release safety
# ---------------------------------------------------------------------------
echo "--- 6. Release Workflow Safety ---"
RELEASE_WF="${REPO_ROOT}/.github/workflows/release.yml"
if [ -f "${RELEASE_WF}" ]; then
    if grep -q 'environment:' "${RELEASE_WF}"; then
        ok "release.yml uses environment gating"
    else
        warn "release.yml may not use environment gating"
    fi

    if grep -q 'skip_changelog_check' "${RELEASE_WF}"; then
        ok "release.yml has changelog check toggle"
    else
        warn "release.yml may not validate changelog"
    fi
else
    warn "release.yml not found"
fi
echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo "========================================"
echo "Summary: ${ERRORS} error(s), ${WARNINGS} warning(s)"
echo "========================================"

if [ "${ERRORS}" -gt 0 ]; then
    echo ""
    echo "Critical issues found. Run the setup scripts to fix:"
    echo "  ./scripts/github-setup/apply-branch-protection.sh"
    echo "  ./scripts/github-setup/apply-tag-protection.sh"
    exit 1
else
    echo "All critical checks passed."
    exit 0
fi
