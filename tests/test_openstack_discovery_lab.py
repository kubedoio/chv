import hashlib
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts/openstack-discovery"
VALIDATOR_SPEC = importlib.util.spec_from_file_location(
    "openstack_discovery_validator", ROOT / "scripts/check-cellhv-openstack-discovery.py"
)
VALIDATOR = importlib.util.module_from_spec(VALIDATOR_SPEC)
VALIDATOR_SPEC.loader.exec_module(VALIDATOR)
RUNNER_SPEC = importlib.util.spec_from_file_location(
    "openstack_path_a_runner", SCRIPTS / "run-path-a.py"
)
RUNNER = importlib.util.module_from_spec(RUNNER_SPEC)
RUNNER_SPEC.loader.exec_module(RUNNER)
MARKER_VALUE = "cellhv-openstack-discovery-disposable-v1\n"
COMMIT = "0123456789abcdef0123456789abcdef01234567"
DIGEST = "0123456789abcdef" * 4


class OpenStackDiscoveryLabTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.temp = Path(self.temporary_directory.name)
        self.marker = self.temp / "marker"
        self.marker.write_text(MARKER_VALUE, encoding="utf-8")
        self.inputs = self.temp / "inputs.env"
        self.inputs.write_text(
            "\n".join(
                (
                    "CELLHV_LAB_ID=cellhv-osd-test",
                    "CELLHV_LAB_CREDENTIAL_CLASS=disposable",
                    "CELLHV_RESOURCE_PREFIX=cellhv-osd-",
                    "CELLHV_HOST_DISTRIBUTION=ubuntu-24.04",
                    "CELLHV_ARCHITECTURE=x86_64",
                    "CELLHV_OPENSTACK_RELEASE=2025.1",
                    f"CELLHV_DEVSTACK_COMMIT={COMMIT}",
                    f"CELLHV_NOVA_COMMIT={COMMIT}",
                    "CELLHV_LIBVIRT_VERSION=10.0.0-2ubuntu8.7",
                    "CELLHV_CLOUD_HYPERVISOR_VERSION=v43.0",
                    f"CELLHV_CLOUD_HYPERVISOR_SHA256={DIGEST}",
                    "CELLHV_GUEST_IMAGE_NAME=ubuntu-24.04-server-cloudimg-amd64.img",
                    f"CELLHV_GUEST_IMAGE_SHA256={DIGEST}",
                    "CELLHV_OVMF_PACKAGE_VERSION=2024.02-2",
                    "",
                )
            ),
            encoding="utf-8",
        )

    def prepare_path_a_fixture(self):
        fake_bin = self.temp / "path-a-bin"
        fake_bin.mkdir()
        command = fake_bin / "fixture-command"
        command.write_text(
            "#!/bin/sh\n"
            "case \"$(basename \"$0\")\" in\n"
            "  uname) if [ \"${1:-}\" = -m ]; then echo x86_64; else echo 6.8.0-fixture; fi ;;\n"
            "  openstack) if [ \"${1:-}\" = --version ]; then echo openstack 7.2.0; else echo '[]'; fi ;;\n"
            "  nova-manage) echo 30.0.0 ;;\n"
            "  virsh) if [ \"${3:-}\" = capabilities ]; then echo '<capabilities/>'; else echo ch:///system; fi ;;\n"
            "  dpkg-query) case \"${3:-}\" in libvirt-daemon-system) echo 10.0.0-2ubuntu8.7 ;; ovmf) echo 2024.02-2 ;; *) exit 1 ;; esac ;;\n"
            "  systemctl) [ \"${1:-}\" != is-active ] || echo active ;;\n"
            "  journalctl) echo 'nova-compute libvirt connected to ch:///system' ;;\n"
            "esac\n"
            "exit 0\n",
            encoding="utf-8",
        )
        command.chmod(0o700)
        for name in ("uname", "openstack", "nova-manage", "virsh", "dpkg-query", "systemctl", "journalctl"):
            (fake_bin / name).symlink_to(command)

        checkouts = {}
        for name in ("devstack", "nova"):
            checkout = self.temp / name
            checkout.mkdir()
            subprocess.run(("git", "init", "-q", str(checkout)), check=True)
            subprocess.run(("git", "-C", str(checkout), "config", "user.email", "fixture@example.invalid"), check=True)
            subprocess.run(("git", "-C", str(checkout), "config", "user.name", "fixture"), check=True)
            (checkout / "README").write_text("fixture\n", encoding="utf-8")
            subprocess.run(("git", "-C", str(checkout), "add", "README"), check=True)
            subprocess.run(("git", "-C", str(checkout), "commit", "-qm", "fixture"), check=True)
            checkouts[name] = (checkout, subprocess.check_output(("git", "-C", str(checkout), "rev-parse", "HEAD"), text=True).strip())

        cloud_hypervisor = self.temp / "cloud-hypervisor"
        cloud_hypervisor.write_text("#!/bin/sh\necho cloud-hypervisor v43.0\n", encoding="utf-8")
        cloud_hypervisor.chmod(0o700)
        nova_config = self.temp / "nova.conf"
        nova_config.write_text("[libvirt]\nconnection_uri = ch:///system\n", encoding="utf-8")
        os_release = self.temp / "os-release"
        os_release.write_text('ID=ubuntu\nVERSION_ID="24.04"\n', encoding="utf-8")
        release_marker = self.temp / "openstack-release"
        release_marker.write_text("2025.1\n", encoding="utf-8")
        guest_directory = self.temp / "images"
        guest_directory.mkdir()
        guest_image = guest_directory / "ubuntu-24.04-server-cloudimg-amd64.img"
        guest_image.write_bytes(b"fixture guest image")
        text = self.inputs.read_text(encoding="utf-8")
        text = text.replace(COMMIT, checkouts["devstack"][1], 1)
        text = text.replace(COMMIT, checkouts["nova"][1], 1)
        text = text.replace(DIGEST, hashlib.sha256(cloud_hypervisor.read_bytes()).hexdigest(), 1)
        text = text.replace(DIGEST, hashlib.sha256(guest_image.read_bytes()).hexdigest(), 1)
        self.inputs.write_text(text, encoding="utf-8")
        environment = self.environment(
            PATH=f"{fake_bin}:{os.environ['PATH']}",
            CELLHV_PATH_A_NOVA_CONFIG=str(nova_config),
            CELLHV_PATH_A_CLOUD_HYPERVISOR=str(cloud_hypervisor),
            CELLHV_PATH_A_DEVSTACK_CHECKOUT=str(checkouts["devstack"][0]),
            CELLHV_PATH_A_NOVA_CHECKOUT=str(checkouts["nova"][0]),
            CELLHV_PATH_A_OS_RELEASE=str(os_release),
            CELLHV_PATH_A_OPENSTACK_RELEASE=str(release_marker),
            CELLHV_PATH_A_GUEST_IMAGE_DIRECTORY=str(guest_directory),
        )
        return environment

    def provenance_report(self, destination, manifest, result="blocked"):
        schema_target = self.temp / VALIDATOR.EXECUTION_SCHEMA_PATH
        schema_target.parent.mkdir(parents=True, exist_ok=True)
        schema_target.write_bytes((ROOT / VALIDATOR.EXECUTION_SCHEMA_PATH).read_bytes())
        manifest_relative = destination.relative_to(self.temp) / "execution-manifest.json"
        manifest_target = self.temp / manifest_relative
        evidence = [{
            "id": "path-a-execution-manifest",
            "kind": "execution-manifest",
            "path": manifest_relative.as_posix(),
            "sha256": hashlib.sha256(manifest_target.read_bytes()).hexdigest(),
            "redacted": True,
        }]
        for command in manifest["commands"]:
            artifact = destination / command["artifact"]
            evidence.append({
                "id": command["evidence_id"],
                "kind": "command-output",
                "path": artifact.relative_to(self.temp).as_posix(),
                "sha256": command["sha256"],
                "redacted": True,
                "source_artifact": command["artifact"],
            })
        config = self.temp / "effective-nova.conf"
        config.write_text("[libvirt]\nconnection_uri = ch:///system\n", encoding="utf-8")
        evidence.append({
            "id": "path-a-effective-configuration",
            "kind": "configuration",
            "path": config.relative_to(self.temp).as_posix(),
            "sha256": hashlib.sha256(config.read_bytes()).hexdigest(),
            "redacted": True,
        })
        return {
            "run_id": manifest["run_id"],
            "candidate": "path-a",
            "evidence_status": "partial",
            "result": result,
            "first_success": {"observed": False, "evidence_refs": []},
            "first_failure": {"observed": False, "evidence_refs": []},
            "evidence": evidence,
        }

    def tearDown(self):
        self.temporary_directory.cleanup()

    def environment(self, **overrides):
        environment = os.environ.copy()
        for key in (
            "AWS_ACCESS_KEY_ID",
            "AWS_SECRET_ACCESS_KEY",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "AZURE_CLIENT_SECRET",
            "ARM_CLIENT_SECRET",
            "KUBECONFIG",
        ):
            environment.pop(key, None)
        environment.update(
            {
                "CELLHV_TEST_HOST_MARKER": str(self.marker),
                "CELLHV_PREFLIGHT_TEST_MODE": "1",
                "CELLHV_LAB_CREDENTIAL_CLASS": "disposable",
                "OS_PROJECT_NAME": "cellhv-osd-test",
                "OS_AUTH_URL": "https://identity.test/v3",
            }
        )
        environment.update(overrides)
        return environment

    def run_script(self, name, *arguments, environment=None):
        return subprocess.run(
            [str(SCRIPTS / name), *(str(argument) for argument in arguments)],
            cwd=ROOT,
            env=self.environment() if environment is None else environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_preflight_refuses_missing_marker(self):
        result = self.run_script(
            "preflight.sh",
            self.inputs,
            environment=self.environment(CELLHV_TEST_HOST_MARKER=str(self.temp / "missing")),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("marker is missing", result.stderr)

    def test_preflight_refuses_unpinned_values(self):
        self.inputs.write_text(
            self.inputs.read_text(encoding="utf-8").replace(COMMIT, "CHANGE_ME_40_HEX_COMMIT", 1),
            encoding="utf-8",
        )
        result = self.run_script("preflight.sh", self.inputs)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unresolved input", result.stderr)

    def test_preflight_refuses_marker_override_without_test_mode(self):
        environment = self.environment()
        environment.pop("CELLHV_PREFLIGHT_TEST_MODE")
        result = self.run_script("preflight.sh", self.inputs, environment=environment)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("TEST_MODE=1", result.stderr)

    def test_preflight_refuses_unexpected_manifest_key_and_unsafe_urls(self):
        safe_inputs = self.inputs.read_text(encoding="utf-8")
        self.inputs.write_text(
            safe_inputs + "OS_PASSWORD=production-secret\n",
            encoding="utf-8",
        )
        result = self.run_script("preflight.sh", self.inputs)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected", result.stderr)

        self.inputs.write_text(safe_inputs, encoding="utf-8")
        for url in (
            "file://localhost/etc/passwd",
            "https://user:password@identity.test/v3",
            "https://identity.test/v3#fragment",
            "http://10.0.0.1/v3",
            "https://example.com/v3",
        ):
            with self.subTest(url=url):
                result = self.run_script("preflight.sh", self.inputs, environment=self.environment(OS_AUTH_URL=url))
                self.assertNotEqual(result.returncode, 0)

    def test_preflight_passes_in_isolated_disposable_environment(self):
        result = self.run_script("preflight.sh", self.inputs)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no host state was changed", result.stdout)

    def test_collector_redacts_and_generates_valid_checksums(self):
        source = self.temp / "nova.log"
        source.write_text(
            "password=hunter2\nAuthorization: Bearer abc123\n"
            "endpoint=https://user:pass@identity.test/v3\n",
            encoding="utf-8",
        )
        allowlist = self.temp / "files.tsv"
        allowlist.write_text(f"nova-log\t{source}\n", encoding="utf-8")
        destination = self.temp / "20260721T120000Z-path-a"

        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertEqual(result.returncode, 0, result.stderr)
        collected = (destination / "files/001-nova-log.txt").read_text(encoding="utf-8")
        self.assertNotIn("hunter2", collected)
        self.assertNotIn("abc123", collected)
        self.assertNotIn("user:pass", collected)
        self.assertGreaterEqual(collected.count("[REDACTED]"), 3)

        for line in (destination / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
            expected, relative = line.split(maxsplit=1)
            evidence_file = destination / relative.lstrip("*")
            actual = hashlib.sha256(evidence_file.read_bytes()).hexdigest()
            self.assertEqual(actual, expected)

    def test_collector_redacts_json_secret_values_without_breaking_json(self):
        source = self.temp / "nova.json"
        source.write_text('{"password":"hunter2","nested":{"api_key":"fixture-key"}}\n', encoding="utf-8")
        allowlist = self.temp / "files.tsv"
        allowlist.write_text(f"nova-log\t{source}\n", encoding="utf-8")
        destination = self.temp / "20260721T120004Z-path-a"
        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertEqual(result.returncode, 0, result.stderr)
        value = json.loads((destination / "files/001-nova-log.txt").read_text(encoding="utf-8"))
        self.assertEqual(value["password"], "[REDACTED]")
        self.assertEqual(value["nested"]["api_key"], "[REDACTED]")

    def test_collector_rejects_symlink_and_credential_like_sources(self):
        real_source = self.temp / "nova.log"
        real_source.write_text("safe\n", encoding="utf-8")
        symlink = self.temp / "linked.log"
        symlink.symlink_to(real_source)
        allowlist = self.temp / "files.tsv"
        allowlist.write_text(f"nova-log\t{symlink}\n", encoding="utf-8")
        destination = self.temp / "20260721T120001Z-path-a"
        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(destination.exists())

        credential = self.temp / "openrc-secret.txt"
        credential.write_text("OS_PASSWORD=not-for-collection\n", encoding="utf-8")
        allowlist.write_text(f"config\t{credential}\n", encoding="utf-8")
        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("credential-like", result.stderr)
        self.assertFalse(destination.exists())

    def test_collector_rejects_private_key_content(self):
        source = self.temp / "nova.log"
        source.write_text(
            "-----BEGIN PRIVATE KEY-----\nnot-real-key-data\n-----END PRIVATE KEY-----\n",
            encoding="utf-8",
        )
        allowlist = self.temp / "files.tsv"
        allowlist.write_text(f"nova-log\t{source}\n", encoding="utf-8")
        destination = self.temp / "20260721T120003Z-path-a"
        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("private-key material", result.stderr)
        self.assertFalse(destination.exists())

    def test_cleanup_verifier_passes_and_detects_reserved_process(self):
        clean = self.run_script("verify-cleanup.sh")
        self.assertEqual(clean.returncode, 0, clean.stderr)

        process = subprocess.Popen(
            ["bash", "-c", "exec -a cellhv-osd-residual sleep 30"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        try:
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                dirty = self.run_script("verify-cleanup.sh")
                if dirty.returncode != 0 and "cellhv-osd-residual" not in dirty.stderr:
                    # Output deliberately includes PID/comm only.
                    break
                time.sleep(0.02)
            else:
                self.fail("cleanup verifier did not observe the reserved process")
            dirty = self.run_script("verify-cleanup.sh")
            self.assertNotEqual(dirty.returncode, 0)
            self.assertNotIn("sleep 30", dirty.stderr)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    def test_cleanup_verifier_fails_clearly_when_namespace_inventory_is_denied(self):
        fake_bin = self.temp / "bin"
        fake_bin.mkdir()
        fake_ip = fake_bin / "ip"
        fake_ip.write_text(
            "#!/bin/sh\n"
            "if [ \"${1:-}\" = netns ]; then exit 1; fi\n"
            "exit 0\n",
            encoding="utf-8",
        )
        fake_ip.chmod(0o700)
        environment = self.environment(PATH=f"{fake_bin}:{os.environ['PATH']}")
        result = self.run_script("verify-cleanup.sh", environment=environment)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("namespace inventory was denied or failed", result.stderr)
        self.assertIn("cleanup verification failed", result.stderr)

    def test_collector_rejects_symlink_destination_parent(self):
        source = self.temp / "nova.log"
        source.write_text("safe\n", encoding="utf-8")
        allowlist = self.temp / "files.tsv"
        allowlist.write_text(f"nova-log\t{source}\n", encoding="utf-8")
        real_parent = self.temp / "real-parent"
        real_parent.mkdir()
        linked_parent = self.temp / "linked-parent"
        linked_parent.symlink_to(real_parent, target_is_directory=True)
        destination = linked_parent / "20260721T120002Z-path-a"
        result = self.run_script("collect.sh", self.inputs, allowlist, destination)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((real_parent / destination.name).exists())

    def test_path_a_runner_refuses_without_explicit_execute(self):
        destination = self.temp / "cellhv-osd-fixture"
        result = self.run_script("run-path-a.py", self.inputs, destination)
        self.assertEqual(result.returncode, 2)
        self.assertIn("without --execute", result.stderr)
        self.assertFalse(destination.exists())

    def test_real_mode_refuses_untrusted_binary_and_path_override(self):
        binary = self.temp / "untrusted-binary"
        binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        binary.chmod(0o777)
        with self.assertRaises(RUNNER.ProbeError):
            RUNNER.validated_binary(binary, test_mode=False)
        variable = RUNNER.OVERRIDE_NAMES["cloud_hypervisor"]
        previous = os.environ.get(variable)
        os.environ[variable] = str(binary)
        try:
            with self.assertRaises(RUNNER.ProbeError):
                RUNNER.resolved_paths(test_mode=False)
        finally:
            if previous is None:
                os.environ.pop(variable, None)
            else:
                os.environ[variable] = previous

    def test_path_a_fixture_is_permanently_non_t5(self):
        environment = self.prepare_path_a_fixture()
        destination = self.temp / "cellhv-osd-fixture"
        result = self.run_script("run-path-a.py", "--execute", self.inputs, destination, environment=environment)
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["evidence_class"], "structural-candidate")
        self.assertFalse(manifest["attestation"]["trusted"])
        self.assertTrue(manifest["test_mode"])
        self.assertEqual(manifest["scenario_id"], "OSD-001")
        self.assertEqual(manifest["result"], "candidate-observed")
        self.assertEqual(len(manifest["commands"]), 25)
        self.assertTrue(all((destination / command["artifact"]).is_file() for command in manifest["commands"]))
        self.assertTrue(manifest["observations"]["correlated_nova_libvirt_event"])
        self.assertTrue(manifest["restoration"]["succeeded"])
        self.assertTrue(manifest["cleanup"]["succeeded"])
        schema = json.loads((ROOT / VALIDATOR.EXECUTION_SCHEMA_PATH).read_text(encoding="utf-8"))
        self.assertEqual(VALIDATOR._schema_errors(manifest, schema, schema), [])
        manifest["untrusted_extra_field"] = True
        self.assertTrue(any(
            "unexpected property 'untrusted_extra_field'" in error
            for error in VALIDATOR._schema_errors(manifest, schema, schema)
        ))

    def test_path_a_runner_records_pin_mismatch_and_stops_before_commands(self):
        environment = self.prepare_path_a_fixture()
        lines = self.inputs.read_text(encoding="utf-8").splitlines()
        self.inputs.write_text(
            "\n".join("CELLHV_NOVA_COMMIT=" + "0" * 40 if line.startswith("CELLHV_NOVA_COMMIT=") else line for line in lines) + "\n",
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-pin-mismatch"
        result = self.run_script("run-path-a.py", "--execute", self.inputs, destination, environment=environment)
        self.assertNotEqual(result.returncode, 0)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertIn("nova_revision", manifest["failed_preconditions"])
        self.assertEqual(
            [command["id"] for command in manifest["commands"]], ["cleanup-verifier"]
        )
        self.assertEqual(manifest["result"], "blocked")

    def test_path_a_runner_checks_packages_before_restart(self):
        environment = self.prepare_path_a_fixture()
        self.inputs.write_text(
            self.inputs.read_text(encoding="utf-8").replace(
                "CELLHV_LIBVIRT_VERSION=10.0.0-2ubuntu8.7",
                "CELLHV_LIBVIRT_VERSION=10.0.0-wrong",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-package-mismatch"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertNotEqual(result.returncode, 0)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        command_ids = {command["id"] for command in manifest["commands"]}
        self.assertIn("libvirt-package-version", command_ids)
        self.assertNotIn("nova-compute-restart", command_ids)

    def test_path_a_runner_never_observes_without_correlated_nova_event(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "nova-compute libvirt connected to ch:///system", "nova-compute started"
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-no-correlation"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertNotEqual(result.returncode, 0)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["result"], "blocked")
        self.assertFalse(manifest["observations"]["correlated_nova_libvirt_event"])
        self.assertTrue(manifest["restoration"]["succeeded"])
        self.assertTrue(manifest["cleanup"]["succeeded"])

    def test_path_a_runner_requires_restart_success(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "systemctl) [ \"${1:-}\" != is-active ] || echo active ;;",
                "systemctl) [ \"${1:-}\" != restart ] || exit 1; [ \"${1:-}\" != is-active ] || echo active ;;",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-restart-failure"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertNotEqual(result.returncode, 0)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["result"], "blocked")
        self.assertFalse(manifest["observations"]["restart_succeeded"])

    def test_path_a_runner_redacts_command_output_before_writing(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "nova-compute libvirt connected to ch:///system",
                "nova-compute libvirt connected to ch:///system password=fixture-secret",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-redaction"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        log = (destination / "nova-compute-log.log").read_text(encoding="utf-8")
        self.assertNotIn("fixture-secret", log)
        self.assertIn("[REDACTED]", log)

    def test_path_a_runner_restores_initially_inactive_service(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        state = self.temp / "nova-service-state"
        replacement = (
            f"systemctl) case \"${{1:-}}\" in "
            f"is-active) if [ -f '{state}' ]; then value=$(cat '{state}'); echo \"$value\"; [ \"$value\" = active ]; "
            "else echo inactive; exit 3; fi ;; "
            f"restart|start) echo active > '{state}' ;; stop) echo inactive > '{state}' ;; esac ;;"
        )
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "systemctl) [ \"${1:-}\" != is-active ] || echo active ;;", replacement
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-inactive-restore"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["initial_service_state"], "inactive")
        self.assertTrue(manifest["restoration"]["succeeded"])
        self.assertEqual(state.read_text(encoding="utf-8").strip(), "inactive")

    def test_path_a_runner_does_not_leak_arbitrary_environment(self):
        environment = self.prepare_path_a_fixture()
        environment["CELLHV_SECRET_CANARY"] = "must-not-reach-command"
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "nova-compute libvirt connected to ch:///system",
                "nova-compute libvirt connected to ch:///system ${CELLHV_SECRET_CANARY-unset}",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-minimal-env"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        log = (destination / "nova-compute-log.log").read_text(encoding="utf-8")
        self.assertIn("unset", log)
        self.assertNotIn("must-not-reach-command", log)

    def test_path_a_runner_fails_closed_on_private_key_output(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "nova-compute libvirt connected to ch:///system",
                "-----BEGIN PRIVATE KEY----- fake -----END PRIVATE KEY-----",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-private-key"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((destination / "nova-compute-log.log").exists())
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["result"], "blocked")
        self.assertIn("private-key material", manifest["error"])

    def test_path_a_runner_records_optional_cinder_as_not_exercised(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "openstack) if [ \"${1:-}\" = --version ]; then echo openstack 7.2.0; else echo '[]'; fi ;;",
                "openstack) if [ \"${1:-}\" = --version ]; then echo openstack 7.2.0; "
                "elif [ \"${1:-}\" = volume ]; then echo cinder-not-installed >&2; exit 1; "
                "else echo '[]'; fi ;;",
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-no-cinder"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["result"], "candidate-observed")
        self.assertTrue(manifest["cleanup"]["succeeded"])

    def test_path_a_provenance_requires_exact_id_path_digest_binding(self):
        environment = self.prepare_path_a_fixture()
        destination = self.temp / "cellhv-osd-binding"
        result = self.run_script(
            "run-path-a.py", "--execute", self.inputs, destination, environment=environment
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        report = self.provenance_report(destination, manifest)
        self.assertEqual(VALIDATOR._t5_provenance_errors(self.temp, report), [])
        report["evidence"][1]["source_artifact"] = "wrong.log"
        self.assertTrue(any(
            "path/digest binding mismatch" in error
            for error in VALIDATOR._t5_provenance_errors(self.temp, report)
        ))

    def test_blocked_manifest_cannot_support_viable_result(self):
        environment = self.prepare_path_a_fixture()
        command = self.temp / "path-a-bin" / "fixture-command"
        command.write_text(
            command.read_text(encoding="utf-8").replace(
                "nova-compute libvirt connected to ch:///system", "nova-compute started"
            ),
            encoding="utf-8",
        )
        destination = self.temp / "cellhv-osd-blocked-viable"
        self.run_script("run-path-a.py", "--execute", self.inputs, destination, environment=environment)
        manifest = json.loads((destination / "execution-manifest.json").read_text(encoding="utf-8"))
        report = self.provenance_report(destination, manifest, result="viable")
        self.assertTrue(any(
            "blocked execution manifest cannot support a viable result" in error
            for error in VALIDATOR._t5_provenance_errors(self.temp, report)
        ))


if __name__ == "__main__":
    unittest.main()
