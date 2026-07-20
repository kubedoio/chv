#!/usr/bin/env bash
# Read-only safety and immutable-input checks for Phase A2 discovery.

set -euo pipefail

readonly MARKER_PATH="${CELLHV_TEST_HOST_MARKER:-/etc/cellhv-test-host}"
readonly MARKER_VALUE="cellhv-openstack-discovery-disposable-v1"
readonly INPUTS_PATH="${1:-}"

fail() { printf '[FAIL] %s\n' "$*" >&2; exit 1; }
pass() { printf '[PASS] %s\n' "$*"; }

[[ $# -eq 1 ]] || fail "usage: $0 /absolute/path/to/lab-inputs.env"
if [[ "$MARKER_PATH" != /etc/cellhv-test-host && "${CELLHV_PREFLIGHT_TEST_MODE:-}" != 1 ]]; then
    fail "marker override is allowed only with CELLHV_PREFLIGHT_TEST_MODE=1"
fi
[[ "$INPUTS_PATH" = /* ]] || fail "lab input path must be absolute"
[[ -f "$INPUTS_PATH" && ! -L "$INPUTS_PATH" ]] || fail "lab input must be a regular, non-symlink file"
[[ -r "$MARKER_PATH" && ! -L "$MARKER_PATH" ]] || fail "disposable-host marker is missing or unsafe: $MARKER_PATH"
[[ "$(<"$MARKER_PATH")" == "$MARKER_VALUE" ]] || fail "disposable-host marker has the wrong value"
pass "disposable-host marker validated"

if grep -nEv '^(#.*|$|[A-Z][A-Z0-9_]*=[A-Za-z0-9._:/+-]+)$' "$INPUTS_PATH"; then
    fail "lab input contains unsupported syntax; expansions, quoting, and whitespace are forbidden"
fi

declare -A inputs=()
while IFS='=' read -r key value; do
    [[ -n "$key" && "$key" != \#* ]] || continue
    [[ -z "${inputs[$key]+present}" ]] || fail "lab input contains a duplicate key: $key"
    inputs["$key"]="$value"
done < "$INPUTS_PATH"

required=(
    CELLHV_LAB_ID CELLHV_LAB_CREDENTIAL_CLASS CELLHV_RESOURCE_PREFIX
    CELLHV_HOST_DISTRIBUTION CELLHV_ARCHITECTURE CELLHV_OPENSTACK_RELEASE
    CELLHV_DEVSTACK_COMMIT CELLHV_NOVA_COMMIT CELLHV_LIBVIRT_VERSION
    CELLHV_CLOUD_HYPERVISOR_VERSION CELLHV_CLOUD_HYPERVISOR_SHA256
    CELLHV_GUEST_IMAGE_NAME CELLHV_GUEST_IMAGE_SHA256 CELLHV_OVMF_PACKAGE_VERSION
)
if [[ "${#inputs[@]}" -ne "${#required[@]}" ]]; then
    fail "lab input contains an unexpected or duplicate key"
fi
for key in "${required[@]}"; do
    [[ -n "${inputs[$key]:-}" ]] || fail "missing required input: $key"
    [[ "${inputs[$key]}" != *CHANGE_ME* ]] || fail "unresolved input: $key"
done

[[ "${inputs[CELLHV_LAB_ID]}" == cellhv-osd-* ]] || fail "CELLHV_LAB_ID must use the cellhv-osd- prefix"
[[ "${inputs[CELLHV_LAB_CREDENTIAL_CLASS]}" == disposable ]] || fail "credential class must be disposable"
[[ "${inputs[CELLHV_RESOURCE_PREFIX]}" == cellhv-osd- ]] || fail "resource prefix must be exactly cellhv-osd-"
[[ "${inputs[CELLHV_DEVSTACK_COMMIT]}" =~ ^[0-9a-fA-F]{40}$ ]] || fail "DevStack revision must be a full commit ID"
[[ "${inputs[CELLHV_NOVA_COMMIT]}" =~ ^[0-9a-fA-F]{40}$ ]] || fail "Nova revision must be a full commit ID"
[[ "${inputs[CELLHV_CLOUD_HYPERVISOR_SHA256]}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "Cloud Hypervisor SHA-256 is invalid"
[[ "${inputs[CELLHV_GUEST_IMAGE_SHA256]}" =~ ^[0-9a-fA-F]{64}$ ]] || fail "guest image SHA-256 is invalid"
pass "immutable lab inputs validated"

[[ "${CELLHV_LAB_CREDENTIAL_CLASS:-}" == disposable ]] || fail "export CELLHV_LAB_CREDENTIAL_CLASS=disposable"
[[ "${OS_PROJECT_NAME:-}" == cellhv-osd-* ]] || fail "OS_PROJECT_NAME must use the cellhv-osd- prefix"
[[ -n "${OS_AUTH_URL:-}" ]] || fail "OS_AUTH_URL is required"
python3 - "$OS_AUTH_URL" <<'PY' || fail "OS_AUTH_URL must be HTTPS on private IPv4/.test, or HTTP on loopback"
import ipaddress
import sys
from urllib.parse import urlsplit

try:
    parsed = urlsplit(sys.argv[1])
    if parsed.username is not None or parsed.password is not None or parsed.fragment:
        raise ValueError
    if parsed.scheme not in {"http", "https"} or not parsed.hostname:
        raise ValueError
    if parsed.query:
        raise ValueError
    host = parsed.hostname.lower()
    try:
        address = ipaddress.ip_address(host)
    except ValueError:
        loopback = host == "localhost"
        permitted = loopback or host.endswith(".test")
    else:
        loopback = address.is_loopback
        private_v4 = address.version == 4 and any(
            address in network
            for network in (
                ipaddress.ip_network("10.0.0.0/8"),
                ipaddress.ip_network("172.16.0.0/12"),
                ipaddress.ip_network("192.168.0.0/16"),
            )
        )
        permitted = loopback or private_v4
    if not permitted or (parsed.scheme == "http" and not loopback):
        raise ValueError
    parsed.port
except (ValueError, UnicodeError):
    raise SystemExit(1)
PY

for variable in AWS_ACCESS_KEY_ID AWS_SECRET_ACCESS_KEY GOOGLE_APPLICATION_CREDENTIALS AZURE_CLIENT_SECRET ARM_CLIENT_SECRET KUBECONFIG; do
    [[ -z "${!variable:-}" ]] || fail "production-capable credential variable is present: $variable"
done
pass "credential boundary validated without contacting an endpoint"

for command in git sha256sum awk grep python3; do
    command -v "$command" >/dev/null 2>&1 || fail "required command is missing: $command"
done
pass "required local tools available"

printf '[RESULT] preflight passed; no host state was changed\n'
