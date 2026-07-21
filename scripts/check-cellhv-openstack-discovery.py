#!/usr/bin/env python3
"""Validate CellHV's schema-bound OpenStack discovery evidence report."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from datetime import datetime
from pathlib import Path, PurePosixPath

sys.dont_write_bytecode = True

SCHEMA_PATH = Path("docs/schemas/cellhv-openstack-discovery-report-v1.schema.json")
EXECUTION_SCHEMA_PATH = Path("docs/schemas/cellhv-openstack-path-a-execution-manifest-v1.schema.json")
REPORT_PATH = Path("docs/acceptance/cellhv-openstack-discovery-report-proposed-v1.json")
SECRET_PATTERNS = (
    re.compile(r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----"),
    re.compile(r"(?i)\b(?:password|passwd|secret|token|api[_-]?key|access[_-]?key)\s*[:=]\s*['\"]?(?!<redacted>|redacted|none|null)[^\s,'\"]{6,}"),
    re.compile(r"\bAKIA[0-9A-Z]{16}\b"),
    re.compile(r"\bgh[opsu]_[A-Za-z0-9]{20,}\b"),
    re.compile(r"(?i)\bAuthorization\s*:\s*(?:Bearer|Basic)\s+[^\s,;]+"),
    re.compile(r"(?i)https?://[^\s/@:]+:[^\s/@]+@"),
)
QEMU_IDENTITY_PATTERNS = (
    re.compile(r"(?i)cloud[ _-]?hypervisor\s+(?:is|as|reports? as|identif(?:y|ies|ied) as)\s+qemu\b"),
    re.compile(r"(?i)[\"']?(?:hypervisor_type|vmm_backend|cloud_hypervisor_identity)[\"']?\s*[:=]\s*[\"']qemu[\"']"),
    re.compile(r"(?i)cloud[ _-]?hypervisor.{0,120}(?:through|via|using|as|=|:)\s*(?:the\s+)?qemu:///system"),
)
SUPPORT_CLAIM_PATTERNS = (
    re.compile(r"(?i)\b(?:support[_ -]?claim|support[_ -]?level|status)\s*[:=]\s*['\"]?(?:preview|supported)\b"),
    re.compile(r"(?i)\bOpenStack\s+(?:is\s+)?(?:preview|supported)\b"),
)


def _resolve_ref(schema_root: dict, reference: str) -> dict:
    if not reference.startswith("#/"):
        raise ValueError(f"external schema reference is forbidden: {reference}")
    value: object = schema_root
    for part in reference[2:].split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        if not isinstance(value, dict) or part not in value:
            raise ValueError(f"unresolved schema reference: {reference}")
        value = value[part]
    if not isinstance(value, dict):
        raise ValueError(f"schema reference is not an object: {reference}")
    return value


def _schema_errors(value: object, schema: dict, schema_root: dict, location: str = "$") -> list[str]:
    """Validate the JSON Schema features used by the checked-in report schema."""
    if "$ref" in schema:
        return _schema_errors(value, _resolve_ref(schema_root, schema["$ref"]), schema_root, location)
    errors: list[str] = []
    if "const" in schema and value != schema["const"]:
        errors.append(f"{location}: must equal {schema['const']!r}")
    if "enum" in schema and value not in schema["enum"]:
        errors.append(f"{location}: {value!r} is not an allowed value")
    expected = schema.get("type")
    if expected is None and any(key in schema for key in ("properties", "required", "additionalProperties")):
        expected = "object"
    elif expected is None and any(key in schema for key in ("items", "minItems", "uniqueItems")):
        expected = "array"
    if expected == "object":
        if not isinstance(value, dict):
            return errors + [f"{location}: expected object"]
        properties = schema.get("properties", {})
        for key in schema.get("required", []):
            if key not in value:
                errors.append(f"{location}: missing required property {key!r}")
        if schema.get("additionalProperties") is False:
            for key in value:
                if key not in properties:
                    errors.append(f"{location}: unexpected property {key!r}")
        for key, child in properties.items():
            if key in value:
                errors.extend(_schema_errors(value[key], child, schema_root, f"{location}.{key}"))
    elif expected == "array":
        if not isinstance(value, list):
            return errors + [f"{location}: expected array"]
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{location}: too few items")
        if schema.get("uniqueItems") and len({json.dumps(item, sort_keys=True) for item in value}) != len(value):
            errors.append(f"{location}: items must be unique")
        for index, item in enumerate(value):
            errors.extend(_schema_errors(item, schema.get("items", {}), schema_root, f"{location}[{index}]"))
    elif expected == "string":
        if not isinstance(value, str):
            return errors + [f"{location}: expected string"]
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{location}: shorter than minLength")
        if "pattern" in schema and re.fullmatch(schema["pattern"], value) is None:
            errors.append(f"{location}: does not match {schema['pattern']!r}")
        if schema.get("format") == "date-time":
            try:
                parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
                if parsed.tzinfo is None:
                    raise ValueError
            except ValueError:
                errors.append(f"{location}: must be a timezone-aware ISO-8601 timestamp")
    elif expected == "integer":
        if not isinstance(value, int) or isinstance(value, bool):
            errors.append(f"{location}: expected integer")
        elif value < schema.get("minimum", value):
            errors.append(f"{location}: below minimum")
    elif expected == "boolean" and not isinstance(value, bool):
        errors.append(f"{location}: expected boolean")
    for conditional in schema.get("allOf", []):
        condition_errors = _schema_errors(value, conditional.get("if", {}), schema_root, location)
        branch = conditional.get("then") if not condition_errors else conditional.get("else")
        if branch:
            errors.extend(_schema_errors(value, branch, schema_root, location))
    alternatives = schema.get("anyOf", [])
    if alternatives and all(_schema_errors(value, alternative, schema_root, location) for alternative in alternatives):
        errors.append(f"{location}: must match at least one anyOf alternative")
    return errors


def _scan_text(text: str, location: str, errors: list[str]) -> None:
    for pattern in SECRET_PATTERNS:
        if pattern.search(text):
            errors.append(f"{location}: possible unredacted secret")
            break
    for pattern in QEMU_IDENTITY_PATTERNS:
        if pattern.search(text):
            errors.append(f"{location}: Cloud Hypervisor must not be reported as QEMU")
            break
    for pattern in SUPPORT_CLAIM_PATTERNS:
        if pattern.search(text):
            errors.append(f"{location}: discovery evidence must not claim Preview or Supported status")
            break


def _evidence_errors(root: Path, report: dict) -> list[str]:
    errors: list[str] = []
    root = root.resolve()
    known_ids: set[str] = set()
    for index, artifact in enumerate(report.get("evidence", [])):
        location = f"$.evidence[{index}]"
        if not isinstance(artifact, dict):
            continue
        artifact_id = artifact.get("id")
        if isinstance(artifact_id, str):
            if artifact_id in known_ids:
                errors.append(f"{location}.id: duplicate evidence ID")
            known_ids.add(artifact_id)
        raw_path = artifact.get("path")
        if not isinstance(raw_path, str):
            continue
        pure = PurePosixPath(raw_path)
        if pure.is_absolute() or ".." in pure.parts or "." in pure.parts:
            errors.append(f"{location}.path: traversal and absolute paths are forbidden")
            continue
        path = root / Path(*pure.parts)
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(root)
        except (OSError, ValueError):
            errors.append(f"{location}.path: missing or resolves outside the repository")
            continue
        if not resolved.is_file():
            errors.append(f"{location}.path: must reference a regular file")
            continue
        digest = artifact.get("sha256")
        actual = hashlib.sha256(resolved.read_bytes()).hexdigest()
        if digest != actual:
            errors.append(f"{location}.sha256: digest does not match {raw_path}")
        if artifact.get("redacted") is not True:
            errors.append(f"{location}.redacted: checked-in discovery evidence must be redacted")
        _scan_text(resolved.read_text(encoding="utf-8", errors="replace"), raw_path, errors)

    def collect_refs(value: object) -> set[str]:
        refs: set[str] = set()
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "evidence_refs" and isinstance(child, list):
                    refs.update(ref for ref in child if isinstance(ref, str))
                else:
                    refs.update(collect_refs(child))
        elif isinstance(value, list):
            for child in value:
                refs.update(collect_refs(child))
        return refs

    unknown = sorted(collect_refs(report) - known_ids)
    if unknown:
        errors.append(f"report: unknown evidence references: {', '.join(unknown)}")
    return errors


def _complete_report_errors(report: dict) -> list[str]:
    if report.get("evidence_status") != "complete":
        return []

    errors: list[str] = []
    observed_events = []
    for field in ("first_success", "first_failure"):
        event = report.get(field)
        if isinstance(event, dict) and event.get("observed") is True:
            observed_events.append(field)
            if not event.get("evidence_refs"):
                errors.append(f"report.{field}: a complete observed event requires evidence references")
    if not observed_events:
        errors.append("report: complete evidence requires an observed first success or first failure")

    for field in ("libvirt_api_or_xml", "qemu_specific_assumptions"):
        if not report.get(field):
            errors.append(f"report.{field}: complete evidence requires at least one finding")
    for field in ("network_expectation", "storage_expectation", "core_authority_impact"):
        finding = report.get(field)
        if not isinstance(finding, dict) or not finding.get("evidence_refs"):
            errors.append(f"report.{field}: complete evidence requires evidence references")
    if report.get("result") not in {"blocked", "viable", "rejected"}:
        errors.append("report: complete evidence requires a terminal result")
    return errors


def _t5_provenance_errors(root: Path, report: dict) -> list[str]:
    """Validate structural manifests without treating unsigned output as T5 proof."""
    errors: list[str] = []
    evidence = [item for item in report.get("evidence", []) if isinstance(item, dict)]
    manifests = [item for item in evidence if item.get("kind") == "execution-manifest"]
    complete = report.get("evidence_status") == "complete"
    if not manifests:
        return ["report: complete T5 evidence requires exactly one execution-manifest artifact"] if complete else []
    if len(manifests) != 1:
        return ["report: evidence requires exactly one execution-manifest artifact"]
    manifest_path = root / manifests[0].get("path", "")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return [f"report: execution manifest is not valid JSON: {error}"]
    if not isinstance(manifest, dict):
        return ["report: execution manifest must be an object"]
    try:
        execution_schema = json.loads((root / EXECUTION_SCHEMA_PATH).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        return [f"report: cannot load execution manifest schema: {error}"]
    errors.extend(
        f"execution manifest: {error}"
        for error in _schema_errors(manifest, execution_schema, execution_schema)
    )
    expected = {
        "schema_version": 1,
        "run_id": report.get("run_id"),
        "scenario_id": "OSD-001",
        "candidate": report.get("candidate"),
    }
    for key, value in expected.items():
        if manifest.get(key) != value:
            errors.append(f"report: execution manifest {key} must equal {value!r}")
    attestation = manifest.get("lab_attestation")
    if not isinstance(attestation, dict):
        errors.append("report: execution manifest requires a lab attestation")
    else:
        if attestation.get("marker") != "cellhv-openstack-discovery-disposable-v1":
            errors.append("report: execution manifest has no disposable-lab marker attestation")
        if attestation.get("credential_class") != "disposable":
            errors.append("report: execution manifest credential class must be disposable")
        if not re.fullmatch(r"cellhv-osd-[a-z0-9._-]+", str(attestation.get("lab_id", ""))):
            errors.append("report: execution manifest lab ID is invalid")
    checks = manifest.get("checks")
    if not isinstance(checks, dict):
        errors.append("report: execution manifest requires immutable-state checks")
    else:
        if checks.get("nova_connection_uri") != "ch:///system":
            errors.append("report: execution manifest did not verify ch:///system")
        for key in ("devstack_revision", "nova_revision"):
            if not re.fullmatch(r"[a-f0-9]{40}", str(checks.get(key, ""))):
                errors.append(f"report: execution manifest {key} is not pinned")
        if not re.fullmatch(r"[a-f0-9]{64}", str(checks.get("cloud_hypervisor_sha256", ""))):
            errors.append("report: execution manifest Cloud Hypervisor digest is not pinned")
    if not re.fullmatch(r"[a-f0-9]{64}", str(manifest.get("immutable_inputs_sha256", ""))):
        errors.append("report: execution manifest immutable input digest is invalid")
    required_commands = {
        "cloud-hypervisor-version", "host-kernel", "architecture",
        "openstack-client-version", "nova-version", "libvirt-version",
        "libvirt-package-version", "ovmf-package-version", "libvirt-uri",
        "libvirt-capabilities", "nova-compute-initial-state",
        "nova-compute-restart", "nova-compute-active", "nova-compute-log",
        "nova-compute-restore", "nova-compute-restored-state",
        "cleanup-verifier",
        *(f"inventory-{when}-{resource}" for when in ("before", "after") for resource in ("servers", "ports", "networks", "volumes")),
    }
    commands = manifest.get("commands")
    if not isinstance(commands, list):
        commands = []
        errors.append("report: execution manifest commands must be an array")
    command_ids = [item.get("id") for item in commands if isinstance(item, dict)]
    if len(command_ids) != len(set(command_ids)):
        errors.append("report: execution manifest contains duplicate command IDs")
    missing = sorted(required_commands - set(command_ids))
    if missing and (complete or manifest.get("result") == "candidate-observed"):
        errors.append("report: execution manifest is missing commands: " + ", ".join(missing))
    command_evidence = [item for item in evidence if item.get("kind") == "command-output"]
    command_evidence_ids = {item.get("id") for item in command_evidence}
    for index, command in enumerate(commands):
        if not isinstance(command, dict):
            errors.append(f"report: execution manifest command {index} is invalid")
            continue
        if command.get("timed_out") is not False or not isinstance(command.get("exit_status"), int):
            errors.append(f"report: execution manifest command {command.get('id', index)!r} lacks a bounded outcome")
        argv = command.get("argv")
        if not isinstance(argv, list) or not argv or not Path(str(argv[0])).is_absolute():
            errors.append(f"report: execution manifest command {command.get('id')!r} lacks an absolute executable")
        digest = command.get("sha256")
        matches = [item for item in command_evidence if item.get("id") == command.get("evidence_id")]
        if len(matches) != 1:
            errors.append(f"report: command {command.get('id', index)!r} lacks an exact evidence ID binding")
        elif matches[0].get("sha256") != digest or matches[0].get("source_artifact") != command.get("artifact"):
            errors.append(f"report: command {command.get('id', index)!r} has a path/digest binding mismatch")
    if not any(item.get("kind") == "configuration" for item in evidence):
        errors.append("report: complete T5 evidence requires redacted effective configuration")
    observed_refs: set[str] = set()
    for field in ("first_success", "first_failure"):
        event = report.get(field)
        if isinstance(event, dict) and event.get("observed") is True:
            observed_refs.update(event.get("evidence_refs", []))
    if (complete or observed_refs) and not observed_refs.intersection(command_evidence_ids):
        errors.append("report: observed first result must reference a digest-linked command-output artifact")
    observations = manifest.get("observations", {})
    positive = isinstance(observations, dict) and all(observations.get(key) is True for key in (
        "libvirt_connection_reached", "nova_compute_active_after_restart",
        "restart_succeeded", "correlated_nova_libvirt_event",
    ))
    restored = isinstance(manifest.get("restoration"), dict) and manifest["restoration"].get("succeeded") is True
    cleaned = isinstance(manifest.get("cleanup"), dict) and manifest["cleanup"].get("succeeded") is True
    if manifest.get("result") == "candidate-observed" and not (positive and restored and cleaned):
        errors.append("report: candidate-observed requires correlated restart, restoration, and cleanup evidence")
    if manifest.get("result") == "blocked" and report.get("result") == "viable":
        errors.append("report: a blocked execution manifest cannot support a viable result")
    if complete:
        errors.append("report: unsigned structural execution manifest cannot prove complete T5 evidence")
    try:
        started = datetime.fromisoformat(manifest["started_at"].replace("Z", "+00:00"))
        completed = datetime.fromisoformat(manifest["completed_at"].replace("Z", "+00:00"))
        duration = (completed - started).total_seconds()
        if started.tzinfo is None or completed.tzinfo is None or duration < 0 or duration > 144000:
            raise ValueError
    except (KeyError, AttributeError, TypeError, ValueError):
        errors.append("report: execution manifest timestamps are invalid or exceed the OSD-001 timeout")
    return errors


def _source_reference_errors(report: dict) -> list[str]:
    errors: list[str] = []
    assumptions = report.get("qemu_specific_assumptions", [])
    if not isinstance(assumptions, list):
        return errors
    for finding_index, finding in enumerate(assumptions):
        if not isinstance(finding, dict):
            continue
        for source_index, source in enumerate(finding.get("source_refs", [])):
            if not isinstance(source, dict):
                continue
            start, end = source.get("line_start"), source.get("line_end")
            if isinstance(start, int) and isinstance(end, int) and end < start:
                errors.append(
                    "report.qemu_specific_assumptions"
                    f"[{finding_index}].source_refs[{source_index}]: line_end must be >= line_start"
                )
    return errors


def check(root: Path, report_path: Path = REPORT_PATH, schema_path: Path = SCHEMA_PATH) -> list[str]:
    errors: list[str] = []
    try:
        schema = json.loads((root / schema_path).read_text(encoding="utf-8"))
        report = json.loads((root / report_path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return [str(exc)]
    if not isinstance(schema, dict):
        return [f"{schema_path}: schema must be an object"]
    errors.extend(f"{report_path}: {error}" for error in _schema_errors(report, schema, schema))
    if isinstance(report, dict):
        errors.extend(_evidence_errors(root, report))
        errors.extend(_complete_report_errors(report))
        errors.extend(_t5_provenance_errors(root, report))
        errors.extend(_source_reference_errors(report))
        _scan_text(json.dumps(report, sort_keys=True), str(report_path), errors)
        status, result = report.get("evidence_status"), report.get("result")
        try:
            started = datetime.fromisoformat(report["started_at"].replace("Z", "+00:00"))
            completed = datetime.fromisoformat(report["completed_at"].replace("Z", "+00:00"))
        except (KeyError, AttributeError, ValueError):
            # The schema errors above identify missing or malformed timestamps.
            started = completed = None
        if started is not None and completed is not None:
            duration = completed - started
            if duration.total_seconds() < 0:
                errors.append("report: completed_at must not precede started_at")
            elif status in {"partial", "complete"} and duration.total_seconds() > 120 * 60 * 60:
                errors.append("report: partial/complete discovery duration must not exceed 120 hours")
        if status == "complete" and result in {"not-run", "inconclusive"}:
            errors.append("report: complete evidence must have an honest terminal result")
        if status == "partial" and result == "not-run":
            errors.append("report: partial evidence cannot have result not-run")
        if status == "proposed-not-run":
            if report.get("evidence"):
                errors.append("report: proposed-not-run must not attach run evidence")
            for field in ("first_success", "first_failure"):
                event = report.get(field)
                if isinstance(event, dict) and event.get("observed") is not False:
                    errors.append(f"report.{field}: proposed-not-run observations must be false")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--report", type=Path, default=REPORT_PATH)
    parser.add_argument("--schema", type=Path, default=SCHEMA_PATH)
    args = parser.parse_args()
    errors = check(args.root.resolve(), args.report, args.schema)
    if errors:
        print("CellHV OpenStack discovery evidence validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("CellHV OpenStack discovery schema and evidence are valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
