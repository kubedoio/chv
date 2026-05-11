#!/bin/bash
# Sign SHA256SUMS with GPG or cosign if signing secrets are configured.
#
# Usage:
#   ./scripts/release/sign-checksums.sh [SHA256SUMS-file]
#
# Environment:
#   CHV_RELEASE_GPG_KEY        ASCII-armored GPG private key
#   CHV_RELEASE_GPG_PASSPHRASE GPG key passphrase
#   CHV_RELEASE_COSIGN_KEY     Cosign private key
#   CHV_RELEASE_COSIGN_PASSWORD Cosign key password
#
# If no signing secrets are configured, prints a clear TODO and exits 0.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

CHECKSUMS="${1:-${REPO_ROOT}/dist/packages/SHA256SUMS}"

if [[ ! -f "$CHECKSUMS" ]]; then
    echo "Error: Checksums file not found: $CHECKSUMS" >&2
    exit 1
fi

echo "Signing checksums: $CHECKSUMS"

# ---------------------------------------------------------------------------
# GPG signing
# ---------------------------------------------------------------------------
if [[ -n "${CHV_RELEASE_GPG_KEY:-}" ]]; then
    echo "GPG signing key detected — signing with GPG..."

    GPG_HOME="$(mktemp -d)"
    export GNUPGHOME="$GPG_HOME"

    # Import key
    echo "$CHV_RELEASE_GPG_KEY" | gpg --batch --yes --import 2>/dev/null || {
        echo "Warning: Failed to import GPG key" >&2
        rm -rf "$GPG_HOME"
        exit 0
    }

    KEY_ID="$(gpg --list-keys --with-colons | grep '^fpr' | head -1 | cut -d: -f10)"
    if [[ -z "$KEY_ID" ]]; then
        echo "Warning: Could not determine GPG key ID" >&2
        rm -rf "$GPG_HOME"
        exit 0
    fi

    gpg_opts="--batch --yes --detach-sign --armor"
    if [[ -n "${CHV_RELEASE_GPG_PASSPHRASE:-}" ]]; then
        gpg_opts="$gpg_opts --pinentry-mode loopback --passphrase-fd 0"
        echo "$CHV_RELEASE_GPG_PASSPHRASE" | gpg $gpg_opts \
            --local-user "$KEY_ID" \
            --output "${CHECKSUMS}.sig" \
            "$CHECKSUMS" 2>/dev/null || {
            echo "Warning: GPG signing failed" >&2
            rm -rf "$GPG_HOME"
            exit 0
        }
    else
        gpg $gpg_opts \
            --local-user "$KEY_ID" \
            --output "${CHECKSUMS}.sig" \
            "$CHECKSUMS" 2>/dev/null || {
            echo "Warning: GPG signing failed" >&2
            rm -rf "$GPG_HOME"
            exit 0
        }
    fi

    echo "GPG signature created: ${CHECKSUMS}.sig"
    gpg --verify "${CHECKSUMS}.sig" "$CHECKSUMS" 2>/dev/null || true
    rm -rf "$GPG_HOME"
    exit 0
fi

# ---------------------------------------------------------------------------
# Cosign signing (Sigstore)
# ---------------------------------------------------------------------------
if [[ -n "${CHV_RELEASE_COSIGN_KEY:-}" ]]; then
    if ! command -v cosign >/dev/null 2>&1; then
        echo "Cosign key detected but cosign CLI not found — installing..."
        curl -sL https://github.com/sigstore/cosign/releases/latest/download/cosign-linux-amd64 -o /tmp/cosign
        chmod +x /tmp/cosign
        COSIGN_BIN="/tmp/cosign"
    else
        COSIGN_BIN="cosign"
    fi

    echo "Cosign signing key detected — signing with cosign..."

    echo "$CHV_RELEASE_COSIGN_KEY" > /tmp/cosign.key
    if [[ -n "${CHV_RELEASE_COSIGN_PASSWORD:-}" ]]; then
        COSIGN_PASSWORD="$CHV_RELEASE_COSIGN_PASSWORD" "$COSIGN_BIN" sign-blob \
            --key /tmp/cosign.key \
            --output-signature "${CHECKSUMS}.cosign.sig" \
            "$CHECKSUMS" 2>/dev/null || {
            echo "Warning: Cosign signing failed" >&2
            rm -f /tmp/cosign.key
            exit 0
        }
    else
        "$COSIGN_BIN" sign-blob \
            --key /tmp/cosign.key \
            --output-signature "${CHECKSUMS}.cosign.sig" \
            "$CHECKSUMS" 2>/dev/null || {
            echo "Warning: Cosign signing failed" >&2
            rm -f /tmp/cosign.key
            exit 0
        }
    fi

    echo "Cosign signature created: ${CHECKSUMS}.cosign.sig"
    rm -f /tmp/cosign.key
    exit 0
fi

# ---------------------------------------------------------------------------
# No signing configured
# ---------------------------------------------------------------------------
echo ""
echo "============================================================"
echo "  SIGNING NOT CONFIGURED"
echo "============================================================"
echo ""
echo "The SHA256SUMS file was generated but NOT signed."
echo ""
echo "To enable signing, configure one of these secret sets:"
echo ""
echo "  GPG signing:"
echo "    CHV_RELEASE_GPG_KEY        ASCII-armored private key"
echo "    CHV_RELEASE_GPG_PASSPHRASE (optional) key passphrase"
echo ""
echo "  Cosign (Sigstore) signing:"
echo "    CHV_RELEASE_COSIGN_KEY     Cosign private key"
echo "    CHV_RELEASE_COSIGN_PASSWORD (optional) key password"
echo ""
echo "Users can still verify integrity using SHA256 checksums."
echo ""
