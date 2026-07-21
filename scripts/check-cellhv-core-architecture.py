#!/usr/bin/env python3
"""T0 architecture guards for the chv-agent/CellHV Core migration."""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from pathlib import Path


FORBIDDEN_CORE_DEPENDENCIES = {
    "chv-controlplane-service",
    "chv-controlplane-store",
    "chv-controlplane-types",
    "chv-webui-bff",
    "chv-architecture-reconcile",
    "chv-architecture-validate",
}
CORE_MANIFESTS = (
    "cmd/chv-agent/Cargo.toml",
    "crates/chv-agent-core/Cargo.toml",
    "crates/cellhv-core-types/Cargo.toml",
    "crates/cellhv-core-store/Cargo.toml",
    "crates/cellhv-core-operations/Cargo.toml",
    "crates/cellhv-core-api/Cargo.toml",
    "crates/cellhv-nodecache-migration/Cargo.toml",
    "crates/cellhv-core-startup/Cargo.toml",
    "crates/cellhv-core-fs/Cargo.toml",
    "crates/chv-agent-runtime-ch/Cargo.toml",
)
ACTIVE_CODE_ROOTS = ("cmd", "crates", "gen", "proto", "packaging", "scripts")
TEXT_SUFFIXES = {".rs", ".toml", ".proto", ".yaml", ".yml", ".json", ".sh", ".service"}
ALLOWED_PACKAGED_SERVICES = {
    "chv-agent.service",
    "chv-controlplane.service",
    "chv-nwd.service",
    "chv-stord.service",
}
STORE_DEPENDENCIES = {"rusqlite", "sqlx", "libsqlite3-sys"}
CORE_STORE_PACKAGE = "cellhv-core-store"
CORE_OPERATIONS_PACKAGE = "cellhv-core-operations"
STORE_ALLOWED_CORE_DEPENDENCIES = {"cellhv-core-types"}
OPERATIONS_ALLOWED_DEPENDENCIES = {
    "async-channel",
    "cellhv-core-store",
    "cellhv-core-types",
    "serde",
    "serde_json",
    "thiserror",
    "tokio",
}
OPERATION_AUTHORITY_DECLARATION = re.compile(
    r"\b(?:struct|enum|trait|type)\s+"
    r"(?:Operation(?:Engine|Service|Executor|Processor|Manager|Coordinator)|"
    r"OperationJournal|Journal(?:Engine|Service|Executor|Processor|Manager))\b"
)
OPERATION_MODULE_NAME = re.compile(
    r"(?:^|[_-])(?:operation(?:s|[_-](?:engine|service|executor|processor|manager))?|"
    r"journal(?:[_-](?:engine|service|executor|processor|manager))?)(?:$|[_-])"
)


def load_json(path: Path) -> object:
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def validate_required_object(value: object, schema: dict, location: str = "$") -> list[str]:
    """Validate the small JSON-Schema subset used by the Phase A registries."""
    errors: list[str] = []
    expected = schema.get("type")
    if expected == "object":
        if not isinstance(value, dict):
            return [f"{location}: expected object"]
        for key in schema.get("required", []):
            if key not in value:
                errors.append(f"{location}: missing required property {key!r}")
        if schema.get("additionalProperties") is False:
            allowed = set(schema.get("properties", {}))
            for key in value:
                if key not in allowed:
                    errors.append(f"{location}: unexpected property {key!r}")
        for key, child in schema.get("properties", {}).items():
            if key in value:
                errors.extend(validate_required_object(value[key], child, f"{location}.{key}"))
    elif expected == "array":
        if not isinstance(value, list):
            return [f"{location}: expected array"]
        if len(value) < schema.get("minItems", 0):
            errors.append(f"{location}: too few items")
        for index, item in enumerate(value):
            errors.extend(validate_required_object(item, schema.get("items", {}), f"{location}[{index}]"))
    elif expected == "string":
        if not isinstance(value, str):
            return [f"{location}: expected string"]
        if len(value) < schema.get("minLength", 0):
            errors.append(f"{location}: shorter than minLength")
        if "enum" in schema and value not in schema["enum"]:
            errors.append(f"{location}: {value!r} is not an allowed value")
        if "pattern" in schema and not re.fullmatch(schema["pattern"], value):
            errors.append(f"{location}: {value!r} does not match {schema['pattern']!r}")
    elif expected == "integer":
        if not isinstance(value, int) or isinstance(value, bool):
            errors.append(f"{location}: expected integer")
        elif value < schema.get("minimum", value):
            errors.append(f"{location}: below minimum")
    elif expected == "boolean" and not isinstance(value, bool):
        errors.append(f"{location}: expected boolean")
    return errors


def check(root: Path) -> list[str]:
    errors: list[str] = []
    manifests: dict[str, tuple[Path, dict]] = {}

    for manifest_path in root.glob("**/Cargo.toml"):
        if any(part in {"target", ".git"} for part in manifest_path.parts):
            continue
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        package = manifest.get("package", {})
        if package.get("name"):
            manifests[package["name"]] = (manifest_path, manifest)
        bins = manifest.get("bin", [])
        if package.get("name") == "cellhvd" or any(item.get("name") == "cellhvd" for item in bins):
            errors.append(f"{manifest_path.relative_to(root)}: parallel cellhvd binary is forbidden")

    packaged_services = {path.name for path in (root / "packaging/systemd").glob("*.service")}
    unexpected_services = sorted(packaged_services - ALLOWED_PACKAGED_SERVICES)
    if unexpected_services:
        errors.append(f"packaging/systemd: unclassified service(s): {', '.join(unexpected_services)}")
    agent_units = []
    for service in (root / "packaging/systemd").glob("*.service"):
        text = service.read_text(encoding="utf-8")
        if re.search(r"^ExecStart=.*(?:chv-agent|cellhv|core)", text, re.MULTILINE | re.IGNORECASE):
            agent_units.append(service.name)
    if agent_units != ["chv-agent.service"]:
        errors.append(f"packaging/systemd: exactly chv-agent.service must start the Core runtime; got {agent_units}")

    core_packages: set[str] = set()
    for name in manifests:
        if name == "chv-agent" or name.startswith("chv-agent-") or name.startswith("cellhv-core"):
            core_packages.add(name)
    dependency_graph: dict[str, set[str]] = {}
    store_packages: set[str] = set()
    for name, (_, manifest) in manifests.items():
        dependency_names: set[str] = set()
        dependency_tables = [manifest.get("dependencies", {}), manifest.get("build-dependencies", {})]
        for target in manifest.get("target", {}).values():
            dependency_tables.extend((target.get("dependencies", {}), target.get("build-dependencies", {})))
        for table in dependency_tables:
            for dependency_name, declaration in table.items():
                dependency_names.add(dependency_name)
                if isinstance(declaration, dict) and isinstance(declaration.get("package"), str):
                    dependency_names.add(declaration["package"])
        dependency_graph[name] = dependency_names
        if name in core_packages and dependency_names & STORE_DEPENDENCIES:
            store_packages.add(name)

    reachable = set(core_packages)
    pending = list(core_packages)
    referenced_dependencies: set[str] = set()
    while pending:
        dependencies = dependency_graph.get(pending.pop(), set())
        referenced_dependencies.update(dependencies)
        for dependency in dependencies:
            if dependency in manifests and dependency not in reachable:
                reachable.add(dependency)
                pending.append(dependency)
    forbidden_reachable = sorted((reachable | referenced_dependencies) & FORBIDDEN_CORE_DEPENDENCIES)
    if forbidden_reachable:
        errors.append(f"Core dependency graph reaches forbidden package(s): {', '.join(forbidden_reachable)}")

    store_dependents = sorted(
        name for name, dependencies in dependency_graph.items() if CORE_STORE_PACKAGE in dependencies
    )
    unexpected_store_dependents = [name for name in store_dependents if name != CORE_OPERATIONS_PACKAGE]
    if unexpected_store_dependents:
        errors.append(
            f"{CORE_STORE_PACKAGE}: may only be depended on by {CORE_OPERATIONS_PACKAGE}; "
            f"bypassed by {unexpected_store_dependents}"
        )
    if CORE_OPERATIONS_PACKAGE in manifests and CORE_STORE_PACKAGE not in dependency_graph.get(
        CORE_OPERATIONS_PACKAGE, set()
    ):
        errors.append(f"{CORE_OPERATIONS_PACKAGE}: must directly depend on {CORE_STORE_PACKAGE}")
    unexpected_operations_dependencies = sorted(
        dependency_graph.get(CORE_OPERATIONS_PACKAGE, set()) - OPERATIONS_ALLOWED_DEPENDENCIES
    )
    if unexpected_operations_dependencies:
        errors.append(
            f"{CORE_OPERATIONS_PACKAGE}: dependency boundary forbids "
            f"{unexpected_operations_dependencies}"
        )

    store_core_dependencies = dependency_graph.get(CORE_STORE_PACKAGE, set()) & core_packages
    unexpected_store_dependencies = sorted(store_core_dependencies - STORE_ALLOWED_CORE_DEPENDENCIES)
    if unexpected_store_dependencies:
        errors.append(
            f"{CORE_STORE_PACKAGE}: forbidden Core dependency(s) {unexpected_store_dependencies}; "
            f"only {sorted(STORE_ALLOWED_CORE_DEPENDENCIES)} are allowed"
        )

    for relative in CORE_MANIFESTS:
        path = root / relative
        if not path.exists():
            errors.append(f"{relative}: required Core manifest is missing")
            continue
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))

    authority_path = root / "config/cellhv-core-authority-v1.json"
    try:
        authority = load_json(authority_path)
    except (OSError, ValueError) as exc:
        errors.append(f"config/cellhv-core-authority-v1.json: {exc}")
    else:
        expected = {
            "runtime_service": "chv-agent",
            "runtime_binary": "chv-agent",
            "vm_authority_count": 1,
            "durable_vm_store_count": 1,
            "operation_engine_count": 1,
            "vmm_backend": "cloud-hypervisor",
            "qemu_identity": False,
        }
        for key, expected_value in expected.items():
            if authority.get(key) != expected_value:
                errors.append(f"config/cellhv-core-authority-v1.json: {key} must be {expected_value!r}")
        if authority.get("durable_vm_store_count") != len(store_packages):
            errors.append(
                "config/cellhv-core-authority-v1.json: durable_vm_store_count does not match "
                f"store-bearing Core packages {sorted(store_packages)}"
            )

        operation_engines = [
            manifests[name][0].parent.relative_to(root).as_posix()
            for name in core_packages
            if name == CORE_OPERATIONS_PACKAGE
        ]
        escaped_operation_authorities = []
        process_owners = []
        for name in core_packages:
            package_root = manifests[name][0].parent
            if name != CORE_OPERATIONS_PACKAGE and (
                OPERATION_MODULE_NAME.search(name) or OPERATION_MODULE_NAME.search(package_root.name)
            ):
                escaped_operation_authorities.append(package_root.relative_to(root).as_posix())
            for source in package_root.glob("src/**/*.rs"):
                text = source.read_text(encoding="utf-8", errors="replace")
                if name != CORE_OPERATIONS_PACKAGE and (
                    OPERATION_MODULE_NAME.search(source.stem)
                    or OPERATION_AUTHORITY_DECLARATION.search(text)
                ):
                    escaped_operation_authorities.append(source.relative_to(root).as_posix())
                if re.search(r"HashMap\s*<\s*String\s*,\s*VmProcess\s*>", text):
                    process_owners.append(source.relative_to(root).as_posix())
        if authority.get("operation_engine_count") != len(operation_engines):
            errors.append(
                "config/cellhv-core-authority-v1.json: operation_engine_count does not match "
                f"operation engines {operation_engines}"
            )
        if escaped_operation_authorities:
            errors.append(
                f"operation authority must be owned by {CORE_OPERATIONS_PACKAGE}; found "
                f"{sorted(set(escaped_operation_authorities))}"
            )
        if authority.get("vm_process_owner_count") != len(process_owners):
            errors.append(
                "config/cellhv-core-authority-v1.json: vm_process_owner_count does not match "
                f"process owners {process_owners}"
            )

    forbidden_identity = re.compile(r"\bqmp\b|qemu:///system|\bqemu(?!-(?:img|utils|kvm))\b", re.IGNORECASE)
    for directory in ACTIVE_CODE_ROOTS:
        base = root / directory
        if not base.exists():
            continue
        for path in base.rglob("*"):
            if not path.is_file() or path.suffix not in TEXT_SUFFIXES:
                continue
            if path.name == Path(__file__).name:
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            if forbidden_identity.search(text):
                errors.append(f"{path.relative_to(root)}: forbidden Cloud Hypervisor QEMU/QMP identity")

    validations = (
        ("docs/acceptance/cellhv-core-registry-v1.json", "docs/schemas/cellhv-acceptance-registry-v1.schema.json"),
        ("docs/qualification/cellhv-core-phase-a-claim.json", "docs/schemas/cellhv-compatibility-claim-v1.schema.json"),
    )
    for document_name, schema_name in validations:
        try:
            document = load_json(root / document_name)
            schema = load_json(root / schema_name)
            errors.extend(f"{document_name}: {error}" for error in validate_required_object(document, schema))
        except (OSError, ValueError) as exc:
            errors.append(f"{document_name}: {exc}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    errors = check(args.root.resolve())
    if errors:
        print("CellHV Core architecture guard failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("CellHV Core architecture and registry guards passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
