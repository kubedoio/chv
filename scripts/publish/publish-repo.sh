#!/bin/bash
# CHV Package Repository Publisher
# ==================================
# Generates apt and yum repository metadata from built packages and uploads
# to a configured repository target.
#
# Usage:
#   ./scripts/publish/publish-repo.sh --packages DIR --channel nightly --version VERSION
#
# Options:
#   --packages DIR    Directory containing .deb and .rpm files
#   --channel NAME    Repository channel: nightly, rc, stable
#   --version VER     Package version string for metadata
#
# Environment (all optional — script dry-runs if none are set):
#   CHV_REPO_URL              Base URL of the repository (for metadata)
#   CHV_REPO_S3_BUCKET        S3 bucket for upload
#   CHV_REPO_S3_PREFIX        S3 key prefix (default: chv/)
#   CHV_REPO_RSYNC_TARGET     rsync destination (e.g., user@host:/var/www/repo)
#   CHV_REPO_GPG_KEY          ASCII-armored GPG private key for signing
#   CHV_REPO_GPG_PASSPHRASE   GPG key passphrase
#   AWS_ACCESS_KEY_ID         AWS credential
#   AWS_SECRET_ACCESS_KEY     AWS credential
#
# Safety:
#   - Prints what it would do when credentials are missing
#   - Never deletes existing repository contents
#   - Generates metadata in a temp directory first

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PKGDIR=""
CHANNEL="nightly"
VERSION=""
DRY_RUN=false

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [[ $# -gt 0 ]]; do
    case "$1" in
        --packages) PKGDIR="$2"; shift 2 ;;
        --channel)  CHANNEL="$2"; shift 2 ;;
        --version)  VERSION="$2"; shift 2 ;;
        --dry-run)  DRY_RUN=true; shift ;;
        -h|--help)
            sed -n '2,25p' "$0"
            exit 0
            ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$PKGDIR" || ! -d "$PKGDIR" ]]; then
    echo "Error: --packages directory is required" >&2
    exit 1
fi
if [[ -z "$VERSION" ]]; then
    echo "Error: --version is required" >&2
    exit 1
fi

PKGDIR="$(cd "$PKGDIR" && pwd)"

# ---------------------------------------------------------------------------
# Detect upload capability
# ---------------------------------------------------------------------------
HAS_S3=false
HAS_RSYNC=false
HAS_URL=false

if [[ -n "${CHV_REPO_S3_BUCKET:-}" && -n "${AWS_ACCESS_KEY_ID:-}" && -n "${AWS_SECRET_ACCESS_KEY:-}" ]]; then
    HAS_S3=true
fi
if [[ -n "${CHV_REPO_RSYNC_TARGET:-}" ]]; then
    HAS_RSYNC=true
fi
if [[ -n "${CHV_REPO_URL:-}" ]]; then
    HAS_URL=true
fi

if [[ "$DRY_RUN" == false && "$HAS_S3" == false && "$HAS_RSYNC" == false ]]; then
    echo ""
    echo "============================================================"
    echo "  DRY-RUN MODE — No repository upload credentials configured"
    echo "============================================================"
    echo ""
    echo "Repository metadata will be generated locally but NOT uploaded."
    echo ""
    echo "To enable publishing, configure one of these secret sets:"
    echo ""
    echo "  S3 upload:"
    echo "    CHV_REPO_S3_BUCKET, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY"
    echo ""
    echo "  rsync upload:"
    echo "    CHV_REPO_RSYNC_TARGET"
    echo ""
    echo "  Metadata URL (for generated Release files):"
    echo "    CHV_REPO_URL"
    echo ""
    echo "Optional signing:"
    echo "    CHV_REPO_GPG_KEY, CHV_REPO_GPG_PASSPHRASE"
    echo ""
    DRY_RUN=true
fi

# ---------------------------------------------------------------------------
# Setup GPG for signing if key is provided
# ---------------------------------------------------------------------------
GPG_KEY_ID=""
setup_gpg() {
    if [[ -z "${CHV_REPO_GPG_KEY:-}" ]]; then
        echo "No GPG key configured — repository metadata will be unsigned"
        return 0
    fi

    local gpg_home
    gpg_home="$(mktemp -d)"
    export GNUPGHOME="$gpg_home"

    # Import key
    echo "$CHV_REPO_GPG_KEY" | gpg --batch --yes --import 2>/dev/null || {
        echo "Warning: Failed to import GPG key" >&2
        return 0
    }

    # Get key ID
    GPG_KEY_ID="$(gpg --list-keys --with-colons | grep '^fpr' | head -1 | cut -d: -f10)"
    if [[ -z "$GPG_KEY_ID" ]]; then
        echo "Warning: Could not determine GPG key ID" >&2
        return 0
    fi

    echo "GPG key imported: ${GPG_KEY_ID:0:16}..."
}

# ---------------------------------------------------------------------------
# Generate apt repository
# ---------------------------------------------------------------------------
generate_apt_repo() {
    local repo_dir="$1"
    local apt_dir="${repo_dir}/apt/dists/${CHANNEL}"
    local pool_dir="${repo_dir}/apt/pool/${CHANNEL}"

    echo ""
    echo "=== Generating apt repository ==="

    mkdir -p "$apt_dir/main/binary-amd64"
    mkdir -p "$pool_dir"

    # Copy .deb files into pool
    local deb_count=0
    for deb in "$PKGDIR"/*.deb; do
        [[ -e "$deb" ]] || continue
        cp "$deb" "$pool_dir/"
        deb_count=$((deb_count + 1))
    done
    echo "Copied $deb_count .deb package(s)"

    if [[ $deb_count -eq 0 ]]; then
        echo "Warning: No .deb packages found"
        return 0
    fi

    # Generate Packages file
    dpkg-scanpackages --multiversion "$pool_dir" > "$apt_dir/main/binary-amd64/Packages" 2>/dev/null || {
        echo "Warning: dpkg-scanpackages failed — is dpkg-dev installed?" >&2
        return 0
    }
    gzip -k -f "$apt_dir/main/binary-amd64/Packages"

    # Generate Release file
    local repo_url="${CHV_REPO_URL:-https://example.com/repo}"
    cat > "$apt_dir/Release" <<EOF
Origin: CHV
Label: CHV ${CHANNEL}
Suite: ${CHANNEL}
Codename: ${CHANNEL}
Architectures: amd64
Components: main
Description: CHV ${CHANNEL} repository
Date: $(date -Ru)
EOF

    # Add file hashes to Release
    {
        echo "SHA256:"
        (cd "$repo_dir/apt" && find "dists/${CHANNEL}" -type f | sort | while read -r f; do
            local size
            size="$(stat -c%s "$f")"
            local hash
            hash="$(sha256sum "$f" | cut -d' ' -f1)"
            printf " %s %16d %s\n" "$hash" "$size" "$f"
        done)
    } >> "$apt_dir/Release"

    # Sign Release if GPG key is available
    if [[ -n "$GPG_KEY_ID" ]]; then
        local gpg_opts="--batch --yes --detach-sign --armor"
        if [[ -n "${CHV_REPO_GPG_PASSPHRASE:-}" ]]; then
            gpg_opts="$gpg_opts --pinentry-mode loopback --passphrase-fd 0"
            echo "$CHV_REPO_GPG_PASSPHRASE" | gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --output "$apt_dir/Release.gpg" \
                "$apt_dir/Release" 2>/dev/null || {
                echo "Warning: GPG signing failed" >&2
            }
            echo "$CHV_REPO_GPG_PASSPHRASE" | gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --clear-sign \
                --output "$apt_dir/InRelease" \
                "$apt_dir/Release" 2>/dev/null || {
                echo "Warning: GPG InRelease signing failed" >&2
            }
        else
            gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --output "$apt_dir/Release.gpg" \
                "$apt_dir/Release" 2>/dev/null || {
                echo "Warning: GPG signing failed" >&2
            }
            gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --clear-sign \
                --output "$apt_dir/InRelease" \
                "$apt_dir/Release" 2>/dev/null || {
                echo "Warning: GPG InRelease signing failed" >&2
            }
        fi
        echo "apt repository signed"
    else
        echo "apt repository generated (unsigned)"
    fi

    echo "apt repo location: ${repo_dir}/apt/"
}

# ---------------------------------------------------------------------------
# Generate yum repository
# ---------------------------------------------------------------------------
generate_yum_repo() {
    local repo_dir="$1"
    local yum_dir="${repo_dir}/yum/${CHANNEL}"

    echo ""
    echo "=== Generating yum repository ==="

    mkdir -p "$yum_dir"

    # Copy .rpm files
    local rpm_count=0
    for rpm in "$PKGDIR"/*.rpm; do
        [[ -e "$rpm" ]] || continue
        cp "$rpm" "$yum_dir/"
        rpm_count=$((rpm_count + 1))
    done
    echo "Copied $rpm_count .rpm package(s)"

    if [[ $rpm_count -eq 0 ]]; then
        echo "Warning: No .rpm packages found"
        return 0
    fi

    # Generate repodata
    if command -v createrepo_c >/dev/null 2>&1; then
        createrepo_c --update "$yum_dir" || {
            echo "Warning: createrepo_c failed" >&2
            return 0
        }
    elif command -v createrepo >/dev/null 2>&1; then
        createrepo --update "$yum_dir" || {
            echo "Warning: createrepo failed" >&2
            return 0
        }
    else
        echo "Warning: createrepo_c/createrepo not found — skipping yum metadata generation" >&2
        return 0
    fi

    # Sign repodata if GPG key is available
    if [[ -n "$GPG_KEY_ID" && -f "$yum_dir/repodata/repomd.xml" ]]; then
        local gpg_opts="--batch --yes --detach-sign --armor"
        if [[ -n "${CHV_REPO_GPG_PASSPHRASE:-}" ]]; then
            gpg_opts="$gpg_opts --pinentry-mode loopback --passphrase-fd 0"
            echo "$CHV_REPO_GPG_PASSPHRASE" | gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --output "$yum_dir/repodata/repomd.xml.asc" \
                "$yum_dir/repodata/repomd.xml" 2>/dev/null || {
                echo "Warning: GPG repomd signing failed" >&2
            }
        else
            gpg $gpg_opts \
                --local-user "$GPG_KEY_ID" \
                --output "$yum_dir/repodata/repomd.xml.asc" \
                "$yum_dir/repodata/repomd.xml" 2>/dev/null || {
                echo "Warning: GPG repomd signing failed" >&2
            }
        fi
        echo "yum repository signed"
    fi

    echo "yum repo location: ${repo_dir}/yum/"
}

# ---------------------------------------------------------------------------
# Upload repository
# ---------------------------------------------------------------------------
upload_repo() {
    local repo_dir="$1"

    if [[ "$DRY_RUN" == true ]]; then
        echo ""
        echo "=== Dry-run: would upload the following ==="
        find "$repo_dir" -type f | sort | sed 's/^/  /'
        echo ""
        return 0
    fi

    echo ""
    echo "=== Uploading repository ==="

    if [[ "$HAS_S3" == true ]]; then
        local prefix="${CHV_REPO_S3_PREFIX:-chv}"
        echo "Uploading to s3://${CHV_REPO_S3_BUCKET}/${prefix}/ ..."
        if command -v aws >/dev/null 2>&1; then
            aws s3 sync "$repo_dir/apt/" "s3://${CHV_REPO_S3_BUCKET}/${prefix}/apt/" --delete
            aws s3 sync "$repo_dir/yum/" "s3://${CHV_REPO_S3_BUCKET}/${prefix}/yum/" --delete
            echo "S3 upload complete"
        else
            echo "Warning: aws CLI not found — skipping S3 upload" >&2
        fi
    fi

    if [[ "$HAS_RSYNC" == true ]]; then
        echo "Uploading to ${CHV_REPO_RSYNC_TARGET} ..."
        rsync -avz --delete "$repo_dir/apt/" "${CHV_REPO_RSYNC_TARGET}/apt/" || {
            echo "Warning: rsync apt upload failed" >&2
        }
        rsync -avz --delete "$repo_dir/yum/" "${CHV_REPO_RSYNC_TARGET}/yum/" || {
            echo "Warning: rsync yum upload failed" >&2
        }
        echo "rsync upload complete"
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    echo "CHV Package Repository Publisher"
    echo "================================"
    echo "Channel:  $CHANNEL"
    echo "Version:  $VERSION"
    echo "Packages: $PKGDIR"
    echo "Dry-run:  $DRY_RUN"
    echo ""

    local repo_dir
    repo_dir="$(mktemp -d /tmp/chv-repo-publish-XXXXXX)"

    setup_gpg
    generate_apt_repo "$repo_dir"
    generate_yum_repo "$repo_dir"
    upload_repo "$repo_dir"

    # Cleanup temp repo dir (but not in dry-run, so user can inspect)
    if [[ "$DRY_RUN" == false ]]; then
        rm -rf "$repo_dir"
    else
        echo "Dry-run temp directory preserved for inspection: $repo_dir"
    fi

    echo ""
    echo "Publish complete"
}

main "$@"
