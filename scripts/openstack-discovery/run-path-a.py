#!/usr/bin/env python3
"""Run a bounded, unsigned Path A probe on an explicitly disposable lab."""

from __future__ import annotations

import argparse
import configparser
import hashlib
import importlib.util
import json
import os
import re
import stat
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.dont_write_bytecode = True

SCRIPT_DIR = Path(__file__).resolve().parent
REPOSITORY_ROOT = SCRIPT_DIR.parents[1]
SAFE_ID = re.compile(r"^[a-z0-9][a-z0-9._-]{2,127}$")
CORRELATED_EVENT = re.compile(r"(?i)\\b(?:libvirt|virconnect|cloud[ -]?hypervisor)\\b|ch:///system")
PRODUCTION_PATHS = {
    "nova_config": Path("/etc/nova/nova.conf"),
    "cloud_hypervisor": Path("/usr/bin/cloud-hypervisor"),
    "devstack_checkout": Path("/opt/stack/devstack"),
    "nova_checkout": Path("/opt/stack/nova"),
    "os_release": Path("/etc/os-release"),
    "openstack_release": Path("/etc/cellhv-openstack-release"),
    "guest_image_directory": Path("/opt/stack/data/devstack-files/images"),
}
OVERRIDE_NAMES = {
    key: f"CELLHV_PATH_A_{key.upper()}" for key in PRODUCTION_PATHS
}
COMMAND_SPECS = (
    ("host-kernel", "uname", ("-r",), 10),
    ("architecture", "uname", ("-m",), 10),
    ("openstack-client-version", "openstack", ("--version",), 10),
    ("nova-version", "nova-manage", ("--version",), 10),
    ("libvirt-version", "virsh", ("--version",), 10),
    ("libvirt-package-version", "dpkg-query", ("-W", "-f=${Version}\\n", "libvirt-daemon-system"), 10),
    ("ovmf-package-version", "dpkg-query", ("-W", "-f=${Version}\\n", "ovmf"), 10),
    ("libvirt-uri", "virsh", ("-c", "ch:///system", "uri"), 30),
    ("libvirt-capabilities", "virsh", ("-c", "ch:///system", "capabilities"), 30),
)
INVENTORY_SPECS = (
    ("servers", ("server", "list", "--all-projects", "-f", "json"), True),
    ("ports", ("port", "list", "--all-projects", "-f", "json"), True),
    ("networks", ("network", "list", "--long", "-f", "json"), True),
    ("volumes", ("volume", "list", "--all-projects", "-f", "json"), False),
)
ALLOWED_ENVIRONMENT = (
    "OS_AUTH_URL", "OS_USERNAME", "OS_PASSWORD", "OS_PROJECT_NAME",
    "OS_USER_DOMAIN_NAME", "OS_PROJECT_DOMAIN_NAME", "OS_REGION_NAME",
    "OS_IDENTITY_API_VERSION",
)


class ProbeError(RuntimeError):
    pass


def now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def load_collector():
    spec = importlib.util.spec_from_file_location("cellhv_collect", SCRIPT_DIR / "collect.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def read_inputs(path: Path) -> dict[str, str]:
    try:
        return load_collector().parse_inputs(path)
    except SystemExit as error:
        raise ProbeError(str(error)) from error


def redact_output(data: bytes) -> bytes:
    if b"\0" in data:
        raise ProbeError("command output contains binary data and cannot be collected")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProbeError("command output is not UTF-8") from error
    try:
        redacted = load_collector().redact(text)
    except SystemExit as error:
        raise ProbeError(str(error)) from error
    return redacted.encode()


def safe_output_directory(path: Path) -> None:
    if not path.is_absolute() or not SAFE_ID.fullmatch(path.name):
        raise ProbeError("output must be an absolute, new directory with a safe run ID")
    details = os.lstat(path.parent)
    if not stat.S_ISDIR(details.st_mode) or stat.S_ISLNK(details.st_mode):
        raise ProbeError("output parent must be a real directory")
    if details.st_mode & stat.S_IWOTH and not details.st_mode & stat.S_ISVTX:
        raise ProbeError("output parent is untrusted")
    path.mkdir(mode=0o700)


def resolved_paths(test_mode: bool) -> dict[str, Path]:
    paths = dict(PRODUCTION_PATHS)
    for key, variable in OVERRIDE_NAMES.items():
        value = os.environ.get(variable)
        if value:
            if not test_mode:
                raise ProbeError(f"{variable} is allowed only in preflight test mode")
            paths[key] = Path(value)
    return paths


def minimal_environment(test_mode: bool) -> dict[str, str]:
    environment = {"LANG": "C.UTF-8", "LC_ALL": "C.UTF-8"}
    for name in ALLOWED_ENVIRONMENT:
        if name in os.environ:
            environment[name] = os.environ[name]
    if test_mode:
        environment["PATH"] = os.environ.get("PATH", "/usr/bin:/bin")
    else:
        environment["PATH"] = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    return environment


def executable(name: str, test_mode: bool, environment: dict[str, str]) -> Path:
    search = subprocess.run(
        ("/usr/bin/which", name), capture_output=True, text=True, timeout=5,
        env=environment, check=False,
    )
    if search.returncode != 0:
        raise ProbeError(f"required command is missing: {name}")
    path = Path(search.stdout.strip())
    details = os.stat(path, follow_symlinks=True)
    if not stat.S_ISREG(details.st_mode):
        raise ProbeError(f"command is not a regular file: {path}")
    if not test_mode and (details.st_uid != 0 or details.st_mode & (stat.S_IWGRP | stat.S_IWOTH)):
        raise ProbeError(f"real-lab command must be root-owned and not group/world writable: {path}")
    return path if test_mode else path.resolve()


def validated_binary(path: Path, test_mode: bool) -> Path:
    if not path.is_absolute():
        raise ProbeError(f"binary path must be absolute: {path}")
    resolved = path.resolve(strict=True)
    details = resolved.stat()
    if not stat.S_ISREG(details.st_mode) or not details.st_mode & stat.S_IXUSR:
        raise ProbeError(f"binary is not an executable regular file: {resolved}")
    if not test_mode and (details.st_uid != 0 or details.st_mode & (stat.S_IWGRP | stat.S_IWOTH)):
        raise ProbeError(f"real-lab binary must be root-owned and not group/world writable: {resolved}")
    return path if test_mode else resolved


def run(
    command_id: str,
    command: tuple[str, ...],
    timeout: int,
    output_directory: Path,
    environment: dict[str, str],
) -> dict[str, object]:
    started = now()
    begin = time.monotonic()
    try:
        completed = subprocess.run(
            command, capture_output=True, timeout=timeout, env=environment, check=False
        )
        status = completed.returncode
        raw = completed.stdout + (b"\n--- stderr ---\n" if completed.stderr else b"") + completed.stderr
        timed_out = False
    except subprocess.TimeoutExpired as error:
        status = -1
        raw = (error.stdout or b"") + b"\n--- TIMEOUT ---\n" + (error.stderr or b"")
        timed_out = True
    content = redact_output(raw)
    artifact = f"{command_id}.log"
    target = output_directory / artifact
    target.write_bytes(content)
    target.chmod(0o600)
    return {
        "id": command_id,
        "evidence_id": f"path-a-command-{command_id}",
        "argv": list(command),
        "started_at": started,
        "completed_at": now(),
        "duration_ms": round((time.monotonic() - begin) * 1000),
        "exit_status": status,
        "timed_out": timed_out,
        "artifact": artifact,
        "sha256": hashlib.sha256(content).hexdigest(),
    }


def git_revision(path: Path, git: Path, environment: dict[str, str]) -> str:
    result = subprocess.run(
        (str(git), "-C", str(path), "rev-parse", "HEAD"),
        capture_output=True, text=True, timeout=10, env=environment, check=False,
    )
    if result.returncode != 0 or not re.fullmatch(r"[0-9a-f]{40}\n?", result.stdout):
        raise ProbeError(f"cannot read pinned revision from {path}")
    return result.stdout.strip()


def read_os_release(path: Path) -> str:
    values = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        key, separator, value = line.partition("=")
        if separator:
            values[key] = value.strip().strip('"')
    if not values.get("ID") or not values.get("VERSION_ID"):
        raise ProbeError("host os-release lacks ID or VERSION_ID")
    return f"{values['ID']}-{values['VERSION_ID']}"


def canonical_inventory(record: dict[str, object], output: Path) -> str:
    if record["exit_status"] != 0 or record["timed_out"]:
        raise ProbeError(f"inventory command failed: {record['id']}")
    try:
        value = json.loads((output / str(record["artifact"])).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ProbeError(f"inventory is not JSON: {record['id']}") from error
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def empty_checks() -> dict[str, str]:
    return {
        key: "" for key in (
            "nova_connection_uri", "devstack_revision", "nova_revision",
            "cloud_hypervisor_sha256", "cloud_hypervisor_version",
            "libvirt_package_version", "ovmf_package_version", "host_distribution",
            "architecture", "openstack_release", "guest_image_name",
            "guest_image_sha256",
        )
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--execute", action="store_true", help="acknowledge nova-compute will be restarted")
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.execute:
        raise ProbeError("refusing to run without --execute")

    test_mode = os.environ.get("CELLHV_PREFLIGHT_TEST_MODE") == "1"
    environment = minimal_environment(test_mode)
    try:
        preflight = subprocess.run(
            (str(SCRIPT_DIR / "preflight.sh"), str(args.inputs.resolve())),
            cwd=REPOSITORY_ROOT, capture_output=True, text=True, timeout=30,
            env={**environment, "CELLHV_LAB_CREDENTIAL_CLASS": os.environ.get("CELLHV_LAB_CREDENTIAL_CLASS", ""),
                 "CELLHV_TEST_HOST_MARKER": os.environ.get("CELLHV_TEST_HOST_MARKER", ""),
                 "CELLHV_PREFLIGHT_TEST_MODE": os.environ.get("CELLHV_PREFLIGHT_TEST_MODE", "")},
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise ProbeError("preflight exceeded its 30 second deadline") from error
    if preflight.returncode != 0:
        raise ProbeError(f"preflight failed: {preflight.stderr.strip()}")

    inputs = read_inputs(args.inputs.resolve())
    paths = resolved_paths(test_mode)
    safe_output_directory(args.output)
    manifest: dict[str, object] = {
        "schema_version": 1,
        "run_id": args.output.name,
        "scenario_id": "OSD-001",
        "candidate": "path-a",
        "evidence_class": "structural-candidate",
        "attestation": {"type": "unsigned", "trusted": False},
        "test_mode": test_mode,
        "lab_attestation": {
            "marker": "cellhv-openstack-discovery-disposable-v1",
            "lab_id": inputs["CELLHV_LAB_ID"],
            "credential_class": inputs["CELLHV_LAB_CREDENTIAL_CLASS"],
        },
        "started_at": now(),
        "completed_at": now(),
        "immutable_inputs_sha256": hashlib.sha256(args.inputs.read_bytes()).hexdigest(),
        "checks": empty_checks(),
        "initial_service_state": "unknown",
        "commands": [],
        "observations": {
            "libvirt_connection_reached": False,
            "nova_compute_active_after_restart": False,
            "restart_succeeded": False,
            "correlated_nova_libvirt_event": False,
        },
        "restoration": {"attempted": False, "succeeded": False, "command_id": ""},
        "cleanup": {"attempted": False, "succeeded": False, "command_id": ""},
        "result": "blocked",
        "error": "",
        "failed_preconditions": [],
    }
    binaries: dict[str, Path] = {}
    initial_inventory: dict[str, str] = {}
    service_mutated = False
    probe_candidate = False
    try:
        for name in ("git", "uname", "openstack", "nova-manage", "virsh", "dpkg-query", "systemctl", "journalctl"):
            binaries[name] = executable(name, test_mode, environment)
        paths["cloud_hypervisor"] = validated_binary(paths["cloud_hypervisor"], test_mode)
        configuration = configparser.ConfigParser(interpolation=None)
        with paths["nova_config"].open(encoding="utf-8") as stream:
            configuration.read_file(stream)
        guest_image = paths["guest_image_directory"] / inputs["CELLHV_GUEST_IMAGE_NAME"]
        checks = {
            "nova_connection_uri": configuration.get("libvirt", "connection_uri", fallback=""),
            "devstack_revision": git_revision(paths["devstack_checkout"], binaries["git"], environment),
            "nova_revision": git_revision(paths["nova_checkout"], binaries["git"], environment),
            "cloud_hypervisor_sha256": hashlib.sha256(paths["cloud_hypervisor"].read_bytes()).hexdigest(),
            "cloud_hypervisor_version": "",
            "libvirt_package_version": "",
            "ovmf_package_version": "",
            "host_distribution": read_os_release(paths["os_release"]),
            "architecture": "",
            "openstack_release": paths["openstack_release"].read_text(encoding="utf-8").strip(),
            "guest_image_name": guest_image.name,
            "guest_image_sha256": hashlib.sha256(guest_image.read_bytes()).hexdigest(),
        }
        manifest["checks"] = checks
        expected = {
            "nova_connection_uri": "ch:///system",
            "devstack_revision": inputs["CELLHV_DEVSTACK_COMMIT"].lower(),
            "nova_revision": inputs["CELLHV_NOVA_COMMIT"].lower(),
            "cloud_hypervisor_sha256": inputs["CELLHV_CLOUD_HYPERVISOR_SHA256"].lower(),
            "host_distribution": inputs["CELLHV_HOST_DISTRIBUTION"],
            "openstack_release": inputs["CELLHV_OPENSTACK_RELEASE"],
            "guest_image_name": inputs["CELLHV_GUEST_IMAGE_NAME"],
            "guest_image_sha256": inputs["CELLHV_GUEST_IMAGE_SHA256"].lower(),
        }
        mismatches = [key for key, value in expected.items() if checks[key] != value]
        if mismatches:
            manifest["failed_preconditions"] = mismatches
            raise ProbeError("immutable lab state mismatch: " + ", ".join(mismatches))

        ch_record = run(
            "cloud-hypervisor-version", (str(paths["cloud_hypervisor"]), "--version"),
            10, args.output, environment,
        )
        manifest["commands"].append(ch_record)
        checks["cloud_hypervisor_version"] = (args.output / str(ch_record["artifact"])).read_text().strip()
        if ch_record["exit_status"] != 0 or inputs["CELLHV_CLOUD_HYPERVISOR_VERSION"] not in checks["cloud_hypervisor_version"]:
            manifest["failed_preconditions"] = ["cloud_hypervisor_version"]
            raise ProbeError("Cloud Hypervisor version mismatch")

        records: dict[str, dict[str, object]] = {}
        for command_id, binary_name, arguments, timeout in COMMAND_SPECS:
            record = run(command_id, (str(binaries[binary_name]), *arguments), timeout, args.output, environment)
            manifest["commands"].append(record)
            records[command_id] = record
            if record["exit_status"] != 0:
                raise ProbeError(f"required precondition command failed: {command_id}")
        checks["architecture"] = (args.output / str(records["architecture"]["artifact"])).read_text().strip()
        checks["libvirt_package_version"] = (args.output / str(records["libvirt-package-version"]["artifact"])).read_text().strip()
        checks["ovmf_package_version"] = (args.output / str(records["ovmf-package-version"]["artifact"])).read_text().strip()
        version_expected = {
            "architecture": inputs["CELLHV_ARCHITECTURE"],
            "libvirt_package_version": inputs["CELLHV_LIBVIRT_VERSION"],
            "ovmf_package_version": inputs["CELLHV_OVMF_PACKAGE_VERSION"],
        }
        mismatches = [key for key, value in version_expected.items() if checks[key] != value]
        if mismatches:
            manifest["failed_preconditions"] = mismatches
            raise ProbeError("installed component version mismatch: " + ", ".join(mismatches))

        for resource, arguments, required in INVENTORY_SPECS:
            command_id = f"inventory-before-{resource}"
            record = run(command_id, (str(binaries["openstack"]), *arguments), 60, args.output, environment)
            manifest["commands"].append(record)
            if not required and record["exit_status"] != 0:
                initial_inventory[resource] = f"not-exercised:{record['exit_status']}"
            else:
                initial_inventory[resource] = canonical_inventory(record, args.output)

        initial = run(
            "nova-compute-initial-state",
            (str(binaries["systemctl"]), "is-active", "devstack@n-cpu.service"),
            30, args.output, environment,
        )
        manifest["commands"].append(initial)
        initial_text = (args.output / str(initial["artifact"])).read_text().strip()
        if initial["exit_status"] == 0 and initial_text == "active":
            manifest["initial_service_state"] = "active"
        elif initial_text == "inactive":
            manifest["initial_service_state"] = "inactive"
        else:
            raise ProbeError("cannot establish initial nova-compute service state")

        restart_started = now()
        service_mutated = True
        restart = run(
            "nova-compute-restart",
            (str(binaries["systemctl"]), "restart", "devstack@n-cpu.service"),
            120, args.output, environment,
        )
        manifest["commands"].append(restart)
        active = run(
            "nova-compute-active",
            (str(binaries["systemctl"]), "is-active", "devstack@n-cpu.service"),
            30, args.output, environment,
        )
        manifest["commands"].append(active)
        log = run(
            "nova-compute-log",
            (str(binaries["journalctl"]), "--unit", "devstack@n-cpu.service", "--since", restart_started, "--no-pager"),
            60, args.output, environment,
        )
        manifest["commands"].append(log)
        log_text = (args.output / str(log["artifact"])).read_text(encoding="utf-8")
        observations = manifest["observations"]
        observations["libvirt_connection_reached"] = records["libvirt-uri"]["exit_status"] == 0
        observations["restart_succeeded"] = restart["exit_status"] == 0 and not restart["timed_out"]
        observations["nova_compute_active_after_restart"] = active["exit_status"] == 0
        observations["correlated_nova_libvirt_event"] = log["exit_status"] == 0 and bool(CORRELATED_EVENT.search(log_text))
        probe_candidate = all(observations.values())
    except (OSError, configparser.Error, subprocess.SubprocessError, ProbeError) as error:
        manifest["error"] = str(error)
    finally:
        if service_mutated:
            try:
                state = str(manifest["initial_service_state"])
                action = "start" if state == "active" else "stop"
                restore = run(
                    "nova-compute-restore",
                    (str(binaries["systemctl"]), action, "devstack@n-cpu.service"),
                    120, args.output, environment,
                )
                manifest["commands"].append(restore)
                verify = run(
                    "nova-compute-restored-state",
                    (str(binaries["systemctl"]), "is-active", "devstack@n-cpu.service"),
                    30, args.output, environment,
                )
                manifest["commands"].append(verify)
                observed_state = (args.output / str(verify["artifact"])).read_text().strip()
                restored = restore["exit_status"] == 0 and (
                    (state == "active" and verify["exit_status"] == 0 and observed_state == "active")
                    or (state == "inactive" and observed_state == "inactive")
                )
                manifest["restoration"] = {
                    "attempted": True, "succeeded": restored,
                    "command_id": "nova-compute-restored-state",
                }
            except (OSError, subprocess.SubprocessError, ProbeError) as error:
                manifest["restoration"] = {
                    "attempted": True, "succeeded": False,
                    "command_id": "nova-compute-restored-state",
                }
                manifest["error"] = "; ".join(filter(None, (str(manifest["error"]), f"restoration: {error}")))
        if initial_inventory:
            try:
                cleanup_ok = True
                for resource, arguments, required in INVENTORY_SPECS:
                    command_id = f"inventory-after-{resource}"
                    record = run(command_id, (str(binaries["openstack"]), *arguments), 60, args.output, environment)
                    manifest["commands"].append(record)
                    try:
                        if not required and initial_inventory[resource].startswith("not-exercised:"):
                            cleanup_ok &= initial_inventory[resource] == f"not-exercised:{record['exit_status']}"
                        else:
                            cleanup_ok &= canonical_inventory(record, args.output) == initial_inventory[resource]
                    except ProbeError:
                        cleanup_ok = False
                manifest["cleanup"] = {
                    "attempted": True, "succeeded": bool(cleanup_ok),
                    "command_id": "inventory-after-volumes",
                }
            except (OSError, subprocess.SubprocessError, ProbeError) as error:
                manifest["cleanup"] = {
                    "attempted": True, "succeeded": False,
                    "command_id": "inventory-after-volumes",
                }
                manifest["error"] = "; ".join(filter(None, (str(manifest["error"]), f"cleanup: {error}")))
        try:
            residue = run(
                "cleanup-verifier",
                (str(SCRIPT_DIR / "verify-cleanup.sh"), "--runner-pid", str(os.getpid())),
                60, args.output, environment,
            )
            manifest["commands"].append(residue)
            inventory_clean = manifest["cleanup"]["succeeded"] is True
            manifest["cleanup"] = {
                "attempted": True,
                "succeeded": inventory_clean and residue["exit_status"] == 0 and not residue["timed_out"],
                "command_id": "cleanup-verifier",
            }
        except (OSError, subprocess.SubprocessError, ProbeError) as error:
            manifest["cleanup"] = {
                "attempted": True, "succeeded": False,
                "command_id": "cleanup-verifier",
            }
            manifest["error"] = "; ".join(filter(None, (str(manifest["error"]), f"cleanup verifier: {error}")))
        restoration = manifest["restoration"]
        cleanup = manifest["cleanup"]
        if probe_candidate and restoration["succeeded"] and cleanup["succeeded"]:
            manifest["result"] = "candidate-observed"
        else:
            manifest["result"] = "blocked"
        manifest["completed_at"] = now()
        encoded = (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode()
        target = args.output / "execution-manifest.json"
        target.write_bytes(encoded)
        target.chmod(0o600)
    print(target)
    return 0 if manifest["result"] == "candidate-observed" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProbeError, OSError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2)
