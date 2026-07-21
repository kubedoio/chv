import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]


class CellHvCorePackagingTests(unittest.TestCase):
    def test_shipped_agent_defaults_are_explicit_and_disabled_by_wiring(self):
        for relative in ("docs/examples/agent.toml", "packaging/config/chv.yaml"):
            text = (ROOT / relative).read_text()
            self.assertIn("core_store_path", text)
            self.assertIn("/var/lib/chv/agent/core.db", text)
            self.assertIn("core_api_socket_path", text)
            self.assertIn("/run/chv/core/core-v1.sock", text)
        policy = (ROOT / "config/cellhv-core-identity-policy-v1.json").read_text()
        self.assertIn('"production_startup_enforced": false', policy)

    def test_agent_unit_has_private_systemd_parents_and_unprivileged_identity(self):
        unit = (ROOT / "packaging/systemd/chv-agent.service").read_text()
        self.assertIn("User=chv", unit)
        self.assertIn("Group=chv", unit)
        self.assertIn("UMask=002", unit)
        self.assertIn("RuntimeDirectory=chv", unit)
        self.assertIn("RuntimeDirectoryMode=0775", unit)
        self.assertIn("StateDirectoryMode=0700", unit)
        self.assertIn("ExecStartPre=+/usr/bin/install -d -m 0700", unit)
        self.assertNotIn("ExecStartPost=", unit)
        self.assertIn("ExecStopPost=-/bin/rm -f /run/chv/agent/api.sock", unit)
        self.assertNotRegex(unit, r"rm\s+-f\s+/run/chv/core/core-v1\.sock")

    def test_standalone_and_packaged_agent_units_have_security_parity(self):
        packaged = (ROOT / "packaging/systemd/chv-agent.service").read_text()
        installer = (ROOT / "scripts/install.sh").read_text()
        generated = installer.split("cat > /etc/systemd/system/chv-agent.service <<'EOF'", 1)[1].split("\nEOF", 1)[0]
        for directive in (
            "User=chv",
            "Group=chv",
            "UMask=002",
            "RuntimeDirectory=chv",
            "RuntimeDirectoryMode=0775",
            "StateDirectory=chv/agent",
            "StateDirectoryMode=0700",
        ):
            self.assertIn(directive, packaged)
            self.assertIn(directive, generated)
        self.assertIn("ExecStartPre=+/usr/bin/install -d -m 0700", generated)
        self.assertNotIn("ExecStartPost=", generated)

    def test_install_surfaces_provision_exact_private_parents(self):
        postinstall = (ROOT / "packaging/scripts/postinstall.sh").read_text()
        tmpfiles = (ROOT / "packaging/tmpfiles/chv-node.conf").read_text()
        for path in ("/var/lib/chv/agent", "/run/chv/core"):
            self.assertIn(path, postinstall)
            self.assertRegex(tmpfiles, rf"d {re.escape(path)} 0700 chv chv -")
        self.assertRegex(tmpfiles, r"d /run/chv/agent 0775 chv chv -")

    def test_no_packaging_surface_removes_core_socket_or_runtime_lease(self):
        surfaces = list((ROOT / "packaging").rglob("*")) + [ROOT / "scripts/install.sh"]
        forbidden = ("/run/chv/core/core-v1.sock", ".cellhv-runtime-authority.lease")
        for path in surfaces:
            if not path.is_file():
                continue
            text = path.read_text(errors="ignore")
            for token in forbidden:
                for line in text.splitlines():
                    if token in line and re.search(r"\b(?:rm|remove|unlink)\b", line):
                        self.fail(f"{path}: destructive Core path line: {line}")


if __name__ == "__main__":
    unittest.main()
