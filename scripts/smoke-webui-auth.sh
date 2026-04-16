#!/bin/bash
# Smoke-test Web UI auth flow through nginx.
# Usage: ./scripts/smoke-webui-auth.sh [base_url]
# Example: ./scripts/smoke-webui-auth.sh http://10.5.199.161

set -euo pipefail

BASE_URL="${1:-http://127.0.0.1}"
USERNAME="${CHV_SMOKE_USER:-admin}"
PASSWORD="${CHV_SMOKE_PASSWORD:-admin}"

echo "==============================================="
echo "CHV Web UI Auth Smoke Test"
echo "Base URL: ${BASE_URL}"
echo "Username: ${USERNAME}"
echo "==============================================="

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

login_headers="${tmp_dir}/login.headers"
login_body="${tmp_dir}/login.body"
me_headers="${tmp_dir}/me.headers"
me_body="${tmp_dir}/me.body"
home_headers="${tmp_dir}/home.headers"
home_body="${tmp_dir}/home.body"
overview_headers="${tmp_dir}/overview.headers"
overview_body="${tmp_dir}/overview.body"

echo "[1/4] POST /api/v1/auth/login"
curl -fsS -D "$login_headers" -o "$login_body" \
  -H 'Content-Type: application/json' \
  -X POST "${BASE_URL}/api/v1/auth/login" \
  -d "{\"username\":\"${USERNAME}\",\"password\":\"${PASSWORD}\"}"

if ! grep -qi '^HTTP/.* 200' "$login_headers"; then
  echo "ERROR: login did not return HTTP 200"
  cat "$login_headers"
  exit 1
fi

if ! grep -qi '^content-type: application/json' "$login_headers"; then
  echo "ERROR: login response is not JSON"
  cat "$login_headers"
  exit 1
fi

token="$(sed -n 's/.*"token":"\([^"]*\)".*/\1/p' "$login_body" | head -n1)"
if [ -z "$token" ]; then
  echo "ERROR: login response did not include a JWT token"
  cat "$login_body"
  exit 1
fi

echo "[2/4] GET /api/v1/auth/me"
curl -fsS -D "$me_headers" -o "$me_body" \
  -H "Authorization: Bearer ${token}" \
  "${BASE_URL}/api/v1/auth/me"

if ! grep -qi '^HTTP/.* 200' "$me_headers"; then
  echo "ERROR: /api/v1/auth/me did not return HTTP 200"
  cat "$me_headers"
  exit 1
fi

if ! grep -qi '^content-type: application/json' "$me_headers"; then
  echo "ERROR: /api/v1/auth/me response is not JSON"
  cat "$me_headers"
  exit 1
fi

if ! grep -q "\"username\":\"${USERNAME}\"" "$me_body"; then
  echo "ERROR: /api/v1/auth/me payload does not contain expected username"
  cat "$me_body"
  exit 1
fi

echo "[3/4] GET /"
curl -fsS -D "$home_headers" -o "$home_body" "${BASE_URL}/"
if ! grep -qi '^HTTP/.* 200' "$home_headers"; then
  echo "ERROR: / did not return HTTP 200"
  cat "$home_headers"
  exit 1
fi
if ! grep -qi '<!doctype html>' "$home_body"; then
  echo "ERROR: / did not return HTML shell"
  head -n 20 "$home_body"
  exit 1
fi

echo "[4/4] GET /overview (legacy route compatibility)"
curl -fsS -D "$overview_headers" -o "$overview_body" "${BASE_URL}/overview"
if ! grep -qi '^HTTP/.* 200' "$overview_headers"; then
  echo "ERROR: /overview did not return HTTP 200"
  cat "$overview_headers"
  exit 1
fi
if grep -qi 'Not found: /overview' "$overview_body"; then
  echo "ERROR: /overview rendered a not-found response"
  head -n 40 "$overview_body"
  exit 1
fi

echo "PASS: auth and route smoke checks succeeded."
