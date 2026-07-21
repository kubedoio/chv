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
        api_source = "crates/cellhv-core-api/src/lib.rs"
        target = root / api_source
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((ROOT / api_source).read_text(), encoding="utf-8")
        authority_source = GUARD.NODECACHE_AUTHORITY_SOURCE
        target = root / authority_source
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((ROOT / authority_source).read_text(), encoding="utf-8")
        identity_source = GUARD.FRESH_IDENTITY_SOURCE
        target = root / identity_source
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text((ROOT / identity_source).read_text(), encoding="utf-8")
        for service in (ROOT / "packaging/systemd").glob("*.service"):
            target = root / "packaging/systemd" / service.name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(service.read_text(), encoding="utf-8")
        for relative in (
            "config/cellhv-core-authority-v1.json",
            "config/cellhv-core-identity-policy-v1.json",
            "docs/specs/adr/019-stable-core-host-identity-and-nodecache-authority.md",
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

    def test_nodecache_authority_cannot_expose_raw_cache(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.NODECACHE_AUTHORITY_SOURCE
            path.write_text(
                path.read_text()
                + "\nimpl NodeCacheAuthority { pub fn leak(&self) -> &NodeCache { todo!() } }\n",
                encoding="utf-8",
            )
            self.assertTrue(
                any("public facade signature exposes" in error for error in GUARD.check(root))
            )

    def test_fresh_identity_authorization_fields_cannot_be_public(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.FRESH_IDENTITY_SOURCE
            path.write_text(
                path.read_text().replace(
                    "    identity: HostIdentity,", "    pub identity: HostIdentity,", 1
                ),
                encoding="utf-8",
            )
            self.assertTrue(
                any("opaque resolver-issued" in error for error in GUARD.check(root))
            )

    def test_nodecache_authority_cannot_accept_mutation_closure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.NODECACHE_AUTHORITY_SOURCE
            path.write_text(
                path.read_text()
                + "\nimpl NodeCacheAuthority { pub fn leak(&mut self, f: impl FnOnce()) { f() } }\n",
                encoding="utf-8",
            )
            self.assertTrue(
                any("public facade signature exposes" in error for error in GUARD.check(root))
            )

    def test_nodecache_authority_rejects_indirect_raw_cache_escapes(self):
        fixtures = (
            "pub type LeakedCache = NodeCache;\n",
            "pub struct Leak { pub cache: NodeCache }\n",
            "impl AsRef<NodeCache> for NodeCacheAuthority { fn as_ref(&self) -> &NodeCache { todo!() } }\n",
            "impl From<NodeCacheAuthority> for NodeCache { fn from(_: NodeCacheAuthority) -> Self { todo!() } }\n",
            "#[derive(Clone)] pub struct NodeCacheAuthority { cache: NodeCache }\n",
            "pub trait CacheExposure { fn cache(&self) -> &NodeCache; } impl CacheExposure for NodeCacheAuthority { fn cache(&self) -> &NodeCache { &self.cache } }\n",
            "impl NodeCacheAuthority { pub const LEAK: fn(&Self) -> &NodeCache = |authority| &authority.cache; }\n",
        )
        for fixture in fixtures:
            with self.subTest(fixture=fixture), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.copy_baseline(root)
                path = root / GUARD.NODECACHE_AUTHORITY_SOURCE
                path.write_text(path.read_text() + "\n" + fixture, encoding="utf-8")
                self.assertTrue(
                    any("facade containment" in error for error in GUARD.check(root))
                )

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
            path.parent.mkdir(parents=True, exist_ok=True)
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

    def test_authoritative_cleanup_cannot_remove_runtime_authority_lease(self):
        for path_name, command in (
            (
                "scripts/install.sh",
                "rm -f /var/lib/chv/agent/.core.db.cellhv-runtime-authority.lease\n",
            ),
            ("packaging/scripts/postremove.sh", "rm -f /var/lib/chv/agent/*.lease\n"),
            ("scripts/dev-install.sh", 'rm -rf "${CHV_DATA_DIR}/agent"\n'),
            ("scripts/install.sh", 'rm -rf "${CHV_DATA_DIR}/agent/.*"\n'),
            ("scripts/install.sh", 'find "${CHV_DATA_DIR}/agent" -delete\n'),
            ("scripts/install.sh", 'rm -rf "${CHV_DATA_DIR}"\n'),
        ):
            with self.subTest(path=path_name, command=command), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.copy_baseline(root)
                path = root / path_name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(command, encoding="utf-8")
                errors = GUARD.check(root)
                self.assertTrue(any("persistent Core runtime authority lease" in error for error in errors))

    def test_safe_authority_parent_setup_is_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "scripts/install.sh"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(
                'mkdir -p "${CHV_DATA_DIR}/agent"\nchmod 0700 "${CHV_DATA_DIR}/agent"\n',
                encoding="utf-8",
            )
            self.assertFalse(
                any("persistent Core runtime authority lease" in error for error in GUARD.check(root))
            )

    def test_undeclared_store_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / GUARD.CORE_MANIFESTS[1]
            text = path.read_text().replace("[dependencies]\n", '[dependencies]\nrusqlite = "1"\n', 1)
            path.write_text(text, encoding="utf-8")
            self.assertTrue(any("durable_vm_store_count" in error for error in GUARD.check(root)))

    def test_agent_cannot_bypass_operations_to_depend_on_store(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/Cargo.toml"
            text = path.read_text().replace(
                "[dependencies]\n",
                '[dependencies]\ncellhv-core-store = { path = "../cellhv-core-store" }\n',
                1,
            )
            path.write_text(text, encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("may only be depended on" in error for error in errors))

    def test_provider_cannot_bypass_operations_to_depend_on_store(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-provider-test/Cargo.toml"
            path.parent.mkdir(parents=True)
            path.write_text(
                '[package]\nname = "chv-agent-provider-test"\nversion = "0.1.0"\n'
                '[dependencies]\ncellhv-core-store = { path = "../cellhv-core-store" }\n',
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("chv-agent-provider-test" in error for error in errors))

    def test_aliased_store_dependency_cannot_bypass_operations(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-runtime-ch/Cargo.toml"
            text = path.read_text().replace(
                "[dependencies]\n",
                '[dependencies]\nauthority = { package = "cellhv-core-store", path = "../cellhv-core-store" }\n',
                1,
            )
            path.write_text(text, encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("chv-agent-runtime-ch" in error for error in errors))

    def test_store_cannot_depend_on_runtime_core_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-store/Cargo.toml"
            text = path.read_text().replace(
                "[dependencies]\n",
                '[dependencies]\nchv-agent-runtime-ch = { path = "../chv-agent-runtime-ch" }\n',
                1,
            )
            path.write_text(text, encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("forbidden Core dependency" in error for error in errors))

    def test_operations_cannot_depend_on_runtime_or_provider_package(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-operations/Cargo.toml"
            text = path.read_text().replace(
                "[dependencies]\n",
                '[dependencies]\nchv-agent-runtime-ch = { path = "../chv-agent-runtime-ch" }\n',
                1,
            )
            path.write_text(text, encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("dependency boundary forbids" in error for error in errors))

    def test_native_api_must_receive_shared_authority_handle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/lib.rs"
            text = path.read_text().replace(
                "pub fn router(authority: AuthorityHandle)",
                "pub fn router(service: OtherHandle)",
                1,
            )
            path.write_text(text, encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("shared AuthorityHandle" in error for error in errors))

    def test_native_api_router_parameter_name_and_qualified_handle_are_allowed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/lib.rs"
            text = path.read_text().replace(
                "pub fn router(authority: AuthorityHandle)",
                "pub fn router(shared: cellhv_core_operations::AuthorityHandle)",
                1,
            )
            path.write_text(text, encoding="utf-8")
            self.assertFalse(
                any("shared AuthorityHandle" in error for error in GUARD.check(root))
            )

    def test_native_api_private_actor_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/lib.rs"
            path.write_text(path.read_text() + "\nstruct DbActor;\n", encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("private DbActor" in error for error in errors))

    def test_native_api_private_actor_in_another_module_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/actor.rs"
            path.write_text("pub struct DbActor;\n", encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("actor.rs: private DbActor" in error for error in errors))

    def test_native_api_direct_operation_service_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/lib.rs"
            path.write_text(
                path.read_text() + "\nfn bypass(_: OperationService) {}\n",
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("must not construct, alias, or own" in error for error in errors))

    def test_transport_cannot_receive_execution_handle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/lib.rs"
            path.write_text(
                path.read_text() + "\npub fn leak(_: ExecutionHandle) {}\n",
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("execution capability is restricted" in error for error in errors))

    def test_runtime_cannot_construct_execution_capability_before_executor_approval(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-runtime-ch/src/process.rs"
            path.write_text(
                path.read_text()
                + "\nfn bypass() { AuthorityActor::spawn_with_execution(); }\n",
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("execution capability is restricted" in error for error in errors))

    def test_native_api_operation_service_alias_in_another_module_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/database.rs"
            path.write_text(
                "use cellhv_core_operations::OperationService as Database;\n",
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("database.rs: transport must not" in error for error in errors))

    def test_native_api_qualified_operation_service_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/src/database.rs"
            path.write_text(
                "fn bypass(_: cellhv_core_operations::OperationService) {}\n",
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("database.rs: transport must not" in error for error in errors))

    def test_native_api_cannot_depend_upward_on_agent_even_for_tests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-api/Cargo.toml"
            path.write_text(
                path.read_text()
                + '\n[dev-dependencies.agent]\npackage = "chv-agent-core"\npath = "../chv-agent-core"\n',
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("dependency direction forbids" in error for error in errors))

    def test_second_operation_engine_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/src/operation_engine.rs"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("pub struct OperationEngine;\n", encoding="utf-8")
            self.assertTrue(any("operation authority must be owned" in error for error in GUARD.check(root)))

    def test_operation_engine_in_innocuously_named_module_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/chv-agent-core/src/executor.rs"
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("pub struct OperationExecutor;\n", encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("operation authority must be owned" in error for error in errors))

    def test_alternate_core_operation_package_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "crates/cellhv-core-operation-runner/Cargo.toml"
            path.parent.mkdir(parents=True)
            path.write_text(
                '[package]\nname = "cellhv-core-operation-runner"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            errors = GUARD.check(root)
            self.assertTrue(any("operation authority must be owned" in error for error in errors))

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

    def test_machine_id_identity_source_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "config/cellhv-core-identity-policy-v1.json"
            value = json.loads(path.read_text())
            value["machine_id_is_host_id_source"] = True
            path.write_text(json.dumps(value), encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("exactly match ADR-019" in error for error in errors))

    def test_identity_policy_cannot_claim_unimplemented_runtime_enforcement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "config/cellhv-core-identity-policy-v1.json"
            value = json.loads(path.read_text())
            value["adr_status"] = "accepted"
            value["production_startup_enforced"] = True
            value["nodecache_authority_mode_enforced"] = True
            path.write_text(json.dumps(value), encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("exactly match ADR-019" in error for error in errors))

    def test_identity_adr_cannot_be_accepted_before_runtime_enforcement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "docs/specs/adr/019-stable-core-host-identity-and-nodecache-authority.md"
            path.write_text(path.read_text().replace("## Status\n\nProposed", "## Status\n\nAccepted", 1))
            errors = GUARD.check(root)
            self.assertTrue(any("status must remain Proposed" in error for error in errors))

    def test_post_cutover_nodecache_vm_authority_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.copy_baseline(root)
            path = root / "config/cellhv-core-identity-policy-v1.json"
            value = json.loads(path.read_text())
            value["post_cutover_vm_writable_store"] = "core-and-nodecache"
            path.write_text(json.dumps(value), encoding="utf-8")
            errors = GUARD.check(root)
            self.assertTrue(any("exactly match ADR-019" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
