#!/usr/bin/env python3
"""Process-level smoke test for chv-agent core-native authority mode."""

from __future__ import annotations

import argparse
import http.client
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


LEASE_CONFLICT = "another CellHV Core authority holds the runtime lease"


class SmokeFailure(RuntimeError):
    """An actionable process-smoke failure."""


class UnixConnection(http.client.HTTPConnection):
    def __init__(self, socket_path: Path, timeout: float = 2.0) -> None:
        super().__init__("localhost", timeout=timeout)
        self.socket_path = socket_path

    def connect(self) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(str(self.socket_path))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeFailure(message)


def log_tail(path: Path, limit: int = 12_000) -> str:
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except OSError as error:
        return f"<unable to read {path}: {error}>"
    if len(content) > limit:
        return f"<last {limit} bytes>\n{content[-limit:]}"
    return content or "<empty>"


def request(socket_path: Path, method: str, path: str) -> tuple[int, Any]:
    connection = UnixConnection(socket_path)
    try:
        connection.request(method, path)
        response = connection.getresponse()
        raw_body = response.read()
        status = response.status
    finally:
        connection.close()
    try:
        body = json.loads(raw_body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        preview = raw_body[:500].decode("utf-8", errors="replace")
        raise SmokeFailure(
            f"{method} {path} returned non-JSON HTTP {status}: {preview!r}"
        ) from error
    return status, body


def failure_with_log(message: str, path: Path) -> SmokeFailure:
    return SmokeFailure(f"{message}\n--- {path} ---\n{log_tail(path)}")


def wait_for_ready(
    process: subprocess.Popen[bytes], socket_path: Path, log_path: Path, timeout: float = 10.0
) -> Any:
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while time.monotonic() < deadline:
        status = process.poll()
        if status is not None:
            raise failure_with_log(
                f"chv-agent exited with status {status} before becoming ready", log_path
            )
        if socket_path.is_socket():
            try:
                response_status, body = request(socket_path, "GET", "/v1/host")
                if response_status == 200:
                    return body
                last_error = SmokeFailure(f"/v1/host returned HTTP {response_status}")
            except (OSError, http.client.HTTPException, SmokeFailure) as error:
                last_error = error
        time.sleep(0.05)
    detail = f": {last_error}" if last_error else ""
    raise failure_with_log(f"chv-agent did not become HTTP-ready within {timeout}s{detail}", log_path)


def process_group_exists(process: subprocess.Popen[bytes]) -> bool:
    try:
        os.killpg(process.pid, 0)
        return True
    except ProcessLookupError:
        return False


def kill_process_group(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=2)
    except subprocess.TimeoutExpired:
        pass


def stop(process: subprocess.Popen[bytes], log_path: Path, timeout: float = 4.0) -> None:
    started = time.monotonic()
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        status = process.wait(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        kill_process_group(process)
        raise failure_with_log(
            f"chv-agent did not stop within the {timeout:.0f}s drain deadline", log_path
        ) from error
    elapsed = time.monotonic() - started
    if process_group_exists(process):
        kill_process_group(process)
        raise failure_with_log("chv-agent left processes in its process group", log_path)
    if status != 0:
        raise failure_with_log(f"chv-agent stopped with status {status}", log_path)
    require(elapsed <= timeout, f"chv-agent stop took {elapsed:.3f}s (limit {timeout:.3f}s)")


def start(binary: Path, config: Path, log_path: Path) -> tuple[subprocess.Popen[bytes], Any]:
    log = log_path.open("wb")
    try:
        process = subprocess.Popen(
            [str(binary), str(config)],
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    finally:
        log.close()
    return process, wait_for_ready(process, config.parent / "core.sock", log_path)


def open_held_connection(socket_path: Path) -> UnixConnection:
    connection = UnixConnection(socket_path)
    connection.request("GET", "/v1/host", headers={"Connection": "keep-alive"})
    response = connection.getresponse()
    body = response.read()
    require(response.status == 200, f"held /v1/host returned HTTP {response.status}")
    try:
        json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        connection.close()
        raise SmokeFailure("held /v1/host returned an invalid JSON body") from error
    require(connection.sock is not None, "server closed the intended held keep-alive connection")
    return connection


def expect_held_connection_closed(connection: UnixConnection) -> None:
    require(connection.sock is not None, "held connection disappeared before shutdown check")
    connection.sock.settimeout(1.0)
    try:
        remaining = connection.sock.recv(1)
    except (ConnectionResetError, BrokenPipeError):
        remaining = b""
    except socket.timeout as error:
        raise SmokeFailure("held keep-alive connection was not drained during shutdown") from error
    finally:
        connection.close()
    require(remaining == b"", "held keep-alive connection remained readable after shutdown")


def write_config(root: Path) -> Path:
    config = root / "agent.toml"
    trap = root / "legacy-runtime-invoked"
    trap_binary = root / "cloud-hypervisor-trap"
    (root / "runtime").mkdir()
    (root / "state").mkdir()
    (root / "runtime").chmod(0o700)
    (root / "state").chmod(0o700)
    trap_binary.write_text(
        f"#!/bin/sh\ntouch '{trap}'\nexit 99\n", encoding="utf-8"
    )
    trap_binary.chmod(0o755)
    config.write_text(
        f"""socket_path = "{root / 'runtime' / 'agent.sock'}"
runtime_dir = "{root / 'runtime'}"
log_level = "info"
control_plane_addr = "https://127.0.0.1:1"
stord_socket = "{root / 'stord.sock'}"
nwd_socket = "{root / 'nwd.sock'}"
chv_binary_path = "{trap_binary}"
stord_binary_path = "/bin/false"
nwd_binary_path = "/bin/false"
cache_path = "{root / 'state' / 'agent-cache.json'}"
authority_mode = "core-native"
core_store_path = "{root / 'state' / 'core.db'}"
core_api_socket_path = "{root / 'core.sock'}"
core_archive_path = "{root / 'state' / 'node-cache-v1.archive'}"
node_id = "core-native-smoke"
""",
        encoding="utf-8",
    )
    return config


def require_expected_state(host: Any, vms: Any) -> None:
    expected_host = {
        "identity": {"id": "core-native-smoke", "resource_version": 1},
        "capabilities": {
            "vm_definitions": False,
            "power_start": False,
            "power_stop": False,
            "power_reboot": False,
            "live_update_vcpus": False,
            "live_update_memory": False,
            "event_watch": False,
        },
    }
    require(host == expected_host, f"unexpected host response: {host!r}")
    require(vms == [], f"unexpected VM response: {vms!r}")


def require_no_runtime_side_effects(root: Path) -> None:
    require(not (root / "legacy-runtime-invoked").exists(), "legacy Cloud Hypervisor runtime was invoked")
    require(not (root / "runtime" / "agent.sock").exists(), "legacy agent socket was created")
    require(not (root / "stord.sock").exists(), "storage daemon socket was created")
    require(not (root / "nwd.sock").exists(), "network daemon socket was created")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/debug/chv-agent"))
    args = parser.parse_args()
    binary = args.binary.resolve()
    require(binary.is_file(), f"chv-agent binary not found: {binary}")

    with tempfile.TemporaryDirectory(prefix="chv-core-native-") as temp:
        root = Path(temp)
        config = write_config(root)
        first_log = root / "first.log"
        contender_log = root / "contender.log"
        restart_log = root / "restart.log"
        recovery_log = root / "recovery.log"
        processes: list[subprocess.Popen[bytes]] = []
        held: UnixConnection | None = None
        try:
            first, host_before = start(binary, config, first_log)
            processes.append(first)
            vm_status, vms_before = request(root / "core.sock", "GET", "/v1/vms")
            require(vm_status == 200, f"GET /v1/vms returned HTTP {vm_status}")
            require_expected_state(host_before, vms_before)

            contender_output = contender_log.open("wb")
            try:
                contender = subprocess.Popen(
                    [str(binary), str(config)],
                    stdout=contender_output,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
            finally:
                contender_output.close()
            processes.append(contender)
            try:
                contender_status = contender.wait(timeout=4)
            except subprocess.TimeoutExpired as error:
                kill_process_group(contender)
                raise failure_with_log("second authority did not reject the runtime lease", contender_log) from error
            require(contender_status != 0, "second authority unexpectedly acquired the runtime lease")
            contender_diagnostic = log_tail(contender_log)
            require(
                LEASE_CONFLICT in contender_diagnostic,
                f"second authority lacked the exact lease-conflict diagnostic\n--- {contender_log} ---\n{contender_diagnostic}",
            )
            require(not (root / "core-2.sock").exists(), "contender created an unexpected second API socket")

            host_status, host_after_conflict = request(root / "core.sock", "GET", "/v1/host")
            vm_status, vms_after_conflict = request(root / "core.sock", "GET", "/v1/vms")
            require(host_status == 200 and vm_status == 200, "first authority stopped responding after lease conflict")
            require_expected_state(host_after_conflict, vms_after_conflict)
            require(first.poll() is None, "first authority exited after rejecting the contender")

            held = open_held_connection(root / "core.sock")
            stop(first, first_log, timeout=4.0)
            expect_held_connection_closed(held)
            held = None
            require(not (root / "core.sock").exists(), "graceful shutdown left the API socket behind")

            restarted, host_after = start(binary, config, restart_log)
            processes.append(restarted)
            vm_status, vms_after = request(root / "core.sock", "GET", "/v1/vms")
            require(vm_status == 200, f"GET /v1/vms after restart returned HTTP {vm_status}")
            require_expected_state(host_after, vms_after)

            os.killpg(restarted.pid, signal.SIGKILL)
            killed_status = restarted.wait(timeout=2)
            require(killed_status == -signal.SIGKILL, f"SIGKILL restart exited with status {killed_status}")
            require((root / "core.sock").is_socket(), "SIGKILL did not leave the expected stale API socket")

            recovered, recovered_host = start(binary, config, recovery_log)
            processes.append(recovered)
            vm_status, recovered_vms = request(root / "core.sock", "GET", "/v1/vms")
            require(vm_status == 200, f"GET /v1/vms after stale-socket recovery returned HTTP {vm_status}")
            require_expected_state(recovered_host, recovered_vms)
            stop(recovered, recovery_log, timeout=4.0)
            require(not (root / "core.sock").exists(), "recovery shutdown left the API socket behind")
            require_no_runtime_side_effects(root)
        except Exception as error:
            diagnostics = "\n".join(
                f"--- {path} ---\n{log_tail(path)}"
                for path in (first_log, contender_log, restart_log, recovery_log)
                if path.exists()
            )
            if diagnostics and diagnostics not in str(error):
                raise SmokeFailure(f"{error}\n{diagnostics}") from error
            raise
        finally:
            if held is not None:
                held.close()
            for process in processes:
                if process.poll() is None or process_group_exists(process):
                    kill_process_group(process)
    print("Core-native chv-agent process acceptance: PASS")
    return 0


if __name__ == "__main__":
    if not __debug__:
        print("error: this smoke test refuses optimized Python execution", file=sys.stderr)
        raise SystemExit(2)
    try:
        raise SystemExit(main())
    except (SmokeFailure, OSError, http.client.HTTPException) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
