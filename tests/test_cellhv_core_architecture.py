import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "cellhv_guard", ROOT / "scripts/check-cellhv-core-architecture.py"
)
GUARD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GUARD)


class ArchitectureGuardTests(unittest.TestCase):
    def copy_baseline(self, root: Path) -> None:
        for relative in GUARD.CORE_MANIFESTS:
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text((ROOT / relative).read_text(), encoding="utf-8")
        process_source = "crates/chv-agent-runtime-ch/src/process.rs"
        target = root / process_source
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((ROOT / process_source).read_text(), encoding="utf-8")
        for service in (ROOT / "packaging/systemd").glob("*.service"):
            target = root / "packaging/systemd" / service.name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(service.read_text(), encoding="utf-8")
        for relative in (
            "config/cellhv-core-authority-v1.json",
            "docs/acceptance/cellhv-core-registry-v1.json",
            "docs/schemas/cellhv-acceptance-registry-v1.schema.json",
            "docs/qualification/cellhv-core-phase-a-claim.json",
            "docs/schemas/cellhv-compatibility-claim-v1.schema.json",
        ):
            target = root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text((ROOT / relative).read_text(), encoding="utf-8")

    def test_repository_passes(self):
        self.assertEqual(GUARD.check(ROOT), [])

    def test_parallel_binary_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "cmd/parallel/Cargo.toml"
            path.parent.mkdir(parents=True)
            path.write_text('[package]\nname = "cellhvd"\nversion = "0.1.0"\n', encoding="utf-8")
            self.assertTrue(any("parallel cellhvd binary" in error for error in GUARD.check(root)))

    def test_forbidden_core_dependency_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.CORE_MANIFESTS[1]
            text = path.read_text()
            text = text.replace(
                "[dependencies]\n",
                '[dependencies]\nchv-controlplane-store = "1"\n',
                1,
            )
            path.write_text(text, encoding="utf-8")
            self.assertTrue(any("forbidden package" in error for error in GUARD.check(root)))

    def test_qemu_identity_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/src/identity.rs"
            path.parent.mkdir(parents=True)
            path.write_text('const URI: &str = "qemu:///system";\n', encoding="utf-8")
            self.assertTrue(any("QEMU/QMP identity" in error for error in GUARD.check(root)))

    def test_second_runtime_service_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            service_dir = root / "packaging/systemd"
            service_dir.mkdir(parents=True, exist_ok=True)
            for name in GUARD.ALLOWED_PACKAGED_SERVICES:
                executable = "chv-agent" if name == "chv-agent.service" else name.removesuffix(".service")
                (service_dir / name).write_text(f"[Service]\nExecStart=/usr/bin/{executable}\n")
            (service_dir / "chv-core.service").write_text("[Service]\nExecStart=/usr/bin/chv-core\n")
            self.assertTrue(any("unclassified service" in error for error in GUARD.check(root)))

    def test_undeclared_store_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.CORE_MANIFESTS[1]
            text = path.read_text().replace("[dependencies]\n", '[dependencies]\nrusqlite = "1"\n', 1)
            path.write_text(text, encoding="utf-8")
            self.assertTrue(any("durable_vm_store_count" in error for error in GUARD.check(root)))

    def test_second_operation_engine_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/src/operation_engine.rs"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("pub struct OperationEngine;\n", encoding="utf-8")
            self.assertTrue(any("operation_engine_count" in error for error in GUARD.check(root)))

    def test_second_process_owner_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/src/second_owner.rs"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("type Owner = HashMap<String, VmProcess>;\n", encoding="utf-8")
            self.assertTrue(any("vm_process_owner_count" in error for error in GUARD.check(root)))

    def test_second_authority_declaration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "config/cellhv-core-authority-v1.json"
            value = json.loads(path.read_text())
            value["vm_authority_count"] = 2
            path.write_text(json.dumps(value), encoding="utf-8")
            self.assertTrue(any("vm_authority_count" in error for error in GUARD.check(root)))


if __name__ == "__main__":
    unittest.main()
