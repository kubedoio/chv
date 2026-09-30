#!/usr/bin/env python3
"""Process-level smoke test for fail-closed security startup (prompt 03, workstream A).

Proves on real processes — not unit tests — that invalid production security
configuration exits cleanly through typed startup handling (#233 successor;
PR #253 landed the typed validation, this script is the process-level proof):

  S1  chv-controlplane with CHV_ALLOW_INSECURE=1 on a production build
      (no `dev` Cargo feature) must exit non-zero with the operator-greppable
      InsecureModeLockedOut guidance and WITHOUT a panic/backtrace.
  S2  chv-controlplane without TLS configuration (and without the bypass)
      must exit non-zero with the "TLS required" fail-closed message — mTLS
      is the default posture — and WITHOUT a panic/backtrace.
  S3  chv-agent with CHV_ALLOW_INSECURE=1 on a production build must exit
      non-zero with the same shape of guidance and WITHOUT a panic/backtrace.

All three scenarios must fail FAST (before listeners/services are up).
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
import time
from pathlib import Path


class SmokeFailure(RuntimeError):
    """An actionable process-smoke failure."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeFailure(message)


# The operator contract strings. #253 established these as greppable runbook
# anchors — the process test pins them so they cannot silently drift.
INSECURE_LOCKOUT_MARKERS = (
    "CHV_ALLOW_INSECURE",
    "'dev' Cargo feature",
    "cargo build --features dev",
)
TLS_REQUIRED_MARKER = "TLS required"
PANIC_MARKERS = ("panicked at", "RUST_BACKTRACE", "stack backtrace")

# Generous ceiling; the gates run before any listener/DB/service wiring, so a
# healthy rejection is sub-second. Anything slower means the gate moved.
STARTUP_REJECT_BUDGET_SECS = 30.0


def run_process(
    binary: Path,
    args: list[str],
    env_extra: dict[str, str],
    label: str,
) -> tuple[int, str, float]:
    """Run a binary to completion; return (exit_code, combined_output, elapsed)."""
    import os

    env = dict(os.environ)
    env.pop("CHV_ALLOW_INSECURE", None)
    env.update(env_extra)

    started = time.monotonic()
    try:
        completed = subprocess.run(
            [str(binary), *args],
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=STARTUP_REJECT_BUDGET_SECS + 15,
        )
    except subprocess.TimeoutExpired as error:
        raise SmokeFailure(
            f"{label}: process did not exit within the timeout — the fail-closed "
            f"startup gate did not reject the configuration"
        ) from error
    elapsed = time.monotonic() - started

    output = (
        (completed.stdout or b"").decode("utf-8", errors="replace")
        + (completed.stderr or b"").decode("utf-8", errors="replace")
    )
    return completed.returncode, output, elapsed


def assert_clean_typed_rejection(
    label: str,
    returncode: int,
    output: str,
    elapsed: float,
    required_markers: tuple[str, ...],
) -> None:
    require(returncode != 0, f"{label}: expected a non-zero exit, got {returncode}")
    for marker in required_markers:
        require(
            marker in output,
            f"{label}: operator-guidance marker {marker!r} missing from output:\n{output[-4000:]}",
        )
    for marker in PANIC_MARKERS:
        require(
            marker not in output,
            f"{label}: output contains panic signature {marker!r} — the rejection "
            f"must be a typed startup error, not a panic:\n{output[-4000:]}",
        )
    require(
        elapsed < STARTUP_REJECT_BUDGET_SECS,
        f"{label}: rejection took {elapsed:.1f}s — the gate must fire before "
        f"listeners/services are constructed",
    )


def write_controlplane_config(root: Path) -> Path:
    """A minimal valid controlplane config with NO TLS section.

    The database is never reached: both security gates fire before
    `connect_pool`. The runtime_dir must be creatable by the invoking user.
    """
    config = f"""
grpc_bind = "127.0.0.1:48443"
http_bind = "127.0.0.1:48080"
log_level = "info"
runtime_dir = "{root}/cp-run"
jwt_secret = "security-startup-smoke-secret-min-32-chars"

[database]
url = "sqlite://{root}/controlplane.db"
migrations_dir = "{root}/migrations"
max_connections = 1
min_connections = 1
acquire_timeout_secs = 5
"""
    path = root / "controlplane.toml"
    path.write_text(config, encoding="utf-8")
    return path


def write_agent_config(root: Path) -> Path:
    """A minimal valid agent config (legacy authority default)."""
    config = f"""
socket_path = "{root}/agent-run/api.sock"
runtime_dir = "{root}/agent-run"
log_level = "info"
control_plane_addr = "https://127.0.0.1:48443"
stord_socket = "{root}/agent-run/stord.sock"
nwd_socket = "{root}/agent-run/nwd.sock"
chv_binary_path = "/usr/bin/cloud-hypervisor"
stord_binary_path = "/usr/bin/chv-stord"
nwd_binary_path = "/usr/bin/chv-nwd"
cache_path = "{root}/agent-run/agent-cache.json"
node_id = "security-startup-smoke-node"
jwt_secret = "security-startup-smoke-secret-min-32-chars"
"""
    path = root / "agent.toml"
    path.write_text(config, encoding="utf-8")
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--controlplane",
        type=Path,
        required=True,
        help="path to a chv-controlplane binary built WITHOUT the dev feature",
    )
    parser.add_argument(
        "--agent",
        type=Path,
        required=True,
        help="path to a chv-agent binary built WITHOUT the dev feature",
    )
    options = parser.parse_args()

    for binary in (options.controlplane, options.agent):
        require(binary.exists(), f"binary not found: {binary}")

    with tempfile.TemporaryDirectory(prefix="chv-secstart-") as raw_root:
        root = Path(raw_root)

        cp_config = write_controlplane_config(root)
        agent_config = write_agent_config(root)

        # S1 — production controlplane refuses the insecure-mode request.
        code, output, elapsed = run_process(
            options.controlplane,
            [str(cp_config)],
            {"CHV_ALLOW_INSECURE": "1"},
            "S1 controlplane/CHV_ALLOW_INSECURE=1",
        )
        assert_clean_typed_rejection(
            "S1 controlplane/CHV_ALLOW_INSECURE=1",
            code,
            output,
            elapsed,
            INSECURE_LOCKOUT_MARKERS,
        )
        print(f"S1 PASS controlplane rejects CHV_ALLOW_INSECURE=1 (exit {code}, {elapsed:.1f}s)")

        # S2 — secure posture is the default: missing TLS fails closed too.
        code, output, elapsed = run_process(
            options.controlplane,
            [str(cp_config)],
            {},
            "S2 controlplane/no-TLS",
        )
        assert_clean_typed_rejection(
            "S2 controlplane/no-TLS", code, output, elapsed, (TLS_REQUIRED_MARKER,)
        )
        print(f"S2 PASS controlplane fails closed without TLS (exit {code}, {elapsed:.1f}s)")

        # S3 — production agent refuses the insecure-mode request.
        code, output, elapsed = run_process(
            options.agent,
            [str(agent_config)],
            {"CHV_ALLOW_INSECURE": "1"},
            "S3 agent/CHV_ALLOW_INSECURE=1",
        )
        assert_clean_typed_rejection(
            "S3 agent/CHV_ALLOW_INSECURE=1",
            code,
            output,
            elapsed,
            INSECURE_LOCKOUT_MARKERS,
        )
        print(f"S3 PASS agent rejects CHV_ALLOW_INSECURE=1 (exit {code}, {elapsed:.1f}s)")

    print("Fail-closed security startup process acceptance: PASS")
    return 0


if __name__ == "__main__":
    if not __debug__:
        print("error: this smoke test refuses optimized Python execution", file=sys.stderr)
        raise SystemExit(2)
    try:
        raise SystemExit(main())
    except (SmokeFailure, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
