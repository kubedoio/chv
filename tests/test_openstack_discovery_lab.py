import hashlib
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


if __name__ == "__main__":
    unittest.main()
