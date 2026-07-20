import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "openstack_discovery", ROOT / "scripts/check-cellhv-openstack-discovery.py"
)
VALIDATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VALIDATOR)


class OpenStackDiscoveryValidatorTests(unittest.TestCase):
    def setUp(self):
        self.schema = json.loads((ROOT / VALIDATOR.SCHEMA_PATH).read_text())
        self.proposal = json.loads((ROOT / VALIDATOR.REPORT_PATH).read_text())

    def write_fixture(self, root: Path, report: dict) -> None:
        schema = root / VALIDATOR.SCHEMA_PATH
        schema.parent.mkdir(parents=True)
        schema.write_text(json.dumps(self.schema), encoding="utf-8")
        target = root / VALIDATOR.REPORT_PATH
        target.parent.mkdir(parents=True)
        target.write_text(json.dumps(report), encoding="utf-8")

    def errors_for(self, mutate=None, artifact_text="redacted discovery output\n"):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report = json.loads(json.dumps(self.proposal))
            if mutate:
                mutate(report, root, artifact_text)
            self.write_fixture(root, report)
            return VALIDATOR.check(root)

    def attach_artifact(self, report: dict, root: Path, text: str, *, path="docs/evidence/openstack/path-a.log"):
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")
        report["evidence_status"] = "partial"
        report["result"] = "blocked"
        report["evidence"] = [{
            "id": "path-a-log",
            "kind": "log",
            "path": path,
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
            "redacted": True,
        }]
        report["configuration"][0]["evidence_refs"] = ["path-a-log"]

    def test_checked_in_proposal_and_schema_pass(self):
        self.assertEqual(VALIDATOR.check(ROOT), [])

    def test_missing_schema_required_field_fails(self):
        errors = self.errors_for(lambda report, _root, _text: report.pop("maintenance_risk"))
        self.assertTrue(any("missing required property 'maintenance_risk'" in error for error in errors))

    def test_schema_rejects_extra_field(self):
        errors = self.errors_for(lambda report, _root, _text: report.update(support_claim="Supported"))
        self.assertTrue(any("unexpected property 'support_claim'" in error for error in errors))

    def test_partial_evidence_with_matching_digest_passes(self):
        self.assertEqual(self.errors_for(self.attach_artifact), [])

    def test_digest_mismatch_fails(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["evidence"][0]["sha256"] = "0" * 64
        self.assertTrue(any("digest does not match" in error for error in self.errors_for(mutate)))

    def test_path_traversal_fails(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["evidence"][0]["path"] = "../secret.log"
        self.assertTrue(any("traversal" in error for error in self.errors_for(mutate)))

    def test_unredacted_marker_fails(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["evidence"][0]["redacted"] = False
        self.assertTrue(any("must be redacted" in error for error in self.errors_for(mutate)))

    def test_secret_in_evidence_fails(self):
        def mutate(report, root, _text):
            self.attach_artifact(report, root, "password=supersecretvalue\n")
        self.assertTrue(any("unredacted secret" in error for error in self.errors_for(mutate)))

    def test_authorization_credentials_in_evidence_fail(self):
        for credential in ("Authorization: Bearer abc123token", "Authorization: Basic dXNlcjpwYXNz"):
            with self.subTest(credential=credential):
                def mutate(report, root, _text, credential=credential):
                    self.attach_artifact(report, root, credential + "\n")
                self.assertTrue(any("unredacted secret" in error for error in self.errors_for(mutate)))

    def test_url_userinfo_in_evidence_fails(self):
        def mutate(report, root, _text):
            self.attach_artifact(report, root, "endpoint=https://user:password@identity.test/v3\n")
        self.assertTrue(any("unredacted secret" in error for error in self.errors_for(mutate)))

    def test_cloud_hypervisor_qemu_identity_fails(self):
        def mutate(report, _root, _text):
            report["first_success"]["summary"] = "Cloud Hypervisor reports as QEMU"
        self.assertTrue(any("must not be reported as QEMU" in error for error in self.errors_for(mutate)))

    def test_qemu_assumption_catalogue_is_allowed(self):
        def mutate(report, _root, _text):
            report["configuration"][0]["summary"] = "Nova contains a QEMU-specific assumption to investigate"
        self.assertEqual(self.errors_for(mutate), [])

    def test_upstream_qemu_uri_assumption_is_allowed(self):
        def mutate(report, _root, _text):
            report["configuration"][0]["summary"] = (
                "Upstream Nova defaults its libvirt connection to qemu:///system; "
                "this source assumption must be catalogued."
            )
        self.assertEqual(self.errors_for(mutate), [])

    def test_cloud_hypervisor_via_qemu_uri_fails(self):
        def mutate(report, _root, _text):
            report["configuration"][0]["summary"] = "Configure Cloud Hypervisor via qemu:///system"
        self.assertTrue(any("must not be reported as QEMU" in error for error in self.errors_for(mutate)))

    def test_preview_or_supported_claim_fails(self):
        for claim in ("OpenStack is Preview", "OpenStack is Supported"):
            with self.subTest(claim=claim):
                def mutate(report, _root, _text, claim=claim):
                    report["recommended_next_step"]["rationale"] = claim
                self.assertTrue(any("must not claim" in error for error in self.errors_for(mutate)))

    def test_proposal_cannot_claim_observed_event(self):
        def mutate(report, _root, _text):
            report["first_success"]["observed"] = True
        self.assertTrue(any("observations must be false" in error for error in self.errors_for(mutate)))

    def test_partial_cannot_remain_not_run(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["result"] = "not-run"
        self.assertTrue(any("partial evidence cannot" in error for error in self.errors_for(mutate)))

    def test_unknown_evidence_reference_fails(self):
        def mutate(report, _root, _text):
            report["configuration"][0]["evidence_refs"] = ["missing"]
        self.assertTrue(any("unknown evidence references" in error for error in self.errors_for(mutate)))

    def test_schema_rejects_invalid_timestamp(self):
        def mutate(report, _root, _text):
            report["recorded_at"] = "2026-07-21"
        self.assertTrue(any("timezone-aware" in error for error in self.errors_for(mutate)))

    def test_schema_conditional_requires_not_run_result_for_proposal(self):
        def mutate(report, _root, _text):
            report["result"] = "blocked"
        errors = self.errors_for(mutate)
        self.assertTrue(any("$.result: must equal 'not-run'" in error for error in errors))

    def test_schema_conditional_requires_partial_evidence(self):
        def mutate(report, _root, _text):
            report["evidence_status"] = "partial"
            report["result"] = "blocked"
        errors = self.errors_for(mutate)
        self.assertTrue(any("$.evidence: too few items" in error for error in errors))

    def test_schema_conditional_requires_complete_evidence(self):
        def mutate(report, _root, _text):
            report["evidence_status"] = "complete"
            report["result"] = "rejected"
        errors = self.errors_for(mutate)
        self.assertTrue(any("$.evidence: too few items" in error for error in errors))

    def test_complete_requires_observed_event_and_discovery_findings(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["evidence_status"] = "complete"
            report["result"] = "rejected"
        errors = self.errors_for(mutate)
        self.assertTrue(any("observed first success or first failure" in error for error in errors))
        self.assertTrue(any("libvirt_api_or_xml" in error for error in errors))
        self.assertTrue(any("qemu_specific_assumptions" in error for error in errors))
        self.assertTrue(any("network_expectation" in error for error in errors))
        self.assertTrue(any("storage_expectation" in error for error in errors))
        self.assertTrue(any("core_authority_impact" in error for error in errors))

    def test_complete_requires_terminal_result(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["evidence_status"] = "complete"
            report["result"] = "inconclusive"
        self.assertTrue(any("terminal result" in error for error in self.errors_for(mutate)))

    def test_source_reference_rejects_unpinned_revision_and_unsafe_path(self):
        def mutate(report, _root, _text):
            report["qemu_specific_assumptions"] = [{
                "summary": "source finding",
                "evidence_refs": ["missing"],
                "source_refs": [{
                    "repository": "https://example.invalid/repository",
                    "revision": "main",
                    "path": "../unsafe.py",
                    "line_start": 10,
                    "line_end": 20,
                }],
            }]
        errors = self.errors_for(mutate)
        self.assertTrue(any("repository" in error and "not an allowed value" in error for error in errors))
        self.assertTrue(any("revision" in error and "does not match" in error for error in errors))
        self.assertTrue(any("path" in error and "does not match" in error for error in errors))

    def test_source_reference_rejects_reversed_line_range(self):
        def mutate(report, _root, _text):
            report["qemu_specific_assumptions"] = [{
                "summary": "source finding",
                "evidence_refs": ["missing"],
                "source_refs": [{
                    "repository": "https://opendev.org/openstack/nova.git",
                    "revision": "0" * 40,
                    "path": "nova/virt/libvirt/driver.py",
                    "line_start": 20,
                    "line_end": 10,
                }],
            }]
        self.assertTrue(any("line_end must be >= line_start" in error for error in self.errors_for(mutate)))

    def test_completed_before_started_fails(self):
        def mutate(report, _root, _text):
            report["started_at"] = "2026-07-22T00:00:00Z"
            report["completed_at"] = "2026-07-21T23:59:59Z"
        self.assertTrue(any("must not precede" in error for error in self.errors_for(mutate)))

    def test_partial_run_over_120_hours_fails(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["started_at"] = "2026-07-20T00:00:00Z"
            report["completed_at"] = "2026-07-25T00:00:01Z"
        self.assertTrue(any("must not exceed 120 hours" in error for error in self.errors_for(mutate)))

    def test_partial_run_at_120_hours_passes(self):
        def mutate(report, root, text):
            self.attach_artifact(report, root, text)
            report["started_at"] = "2026-07-20T00:00:00Z"
            report["completed_at"] = "2026-07-25T00:00:00Z"
        self.assertEqual(self.errors_for(mutate), [])


if __name__ == "__main__":
    unittest.main()
