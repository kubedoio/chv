#!/usr/bin/env python3
"""Example chv-monitor plugin: PostgreSQL readiness check.

Part of the CHV native monitoring campaign (prompt 04, G4 part 1).
This is an EXAMPLE, not an installed component: the local root
administrator copies it into the plugin allowlist directory and pins
its digest in a manifest. See docs/examples/plugins/README.md.

Readiness, not authentication: the plugin execs `pg_isready` (fixed
binary, fixed argv, no shell) which reports whether the server is
accepting connections. It uses NO credentials — PostgreSQL readiness
does not require them, and credentials never belong in monitoring
telemetry or plugin output.
"""

import json
import subprocess
import sys
import time

# --- local administrator configuration (root-owned file) ---------------
HOST = "127.0.0.1"
PORT = 5432
PG_ISREADY = "/usr/bin/pg_isready"
# ------------------------------------------------------------------------

CHECK_ID = "plugin:example.postgres-readiness"

# pg_isready exit codes (upstream documented semantics).
PG_ACCEPTING = 0
PG_REJECTING = 1
PG_NO_RESPONSE = 2
PG_MISCONFIGURED = 3


def main() -> int:
    started = time.monotonic()
    try:
        # No shell, no interpolated strings: fixed argv only.
        result = subprocess.run(
            [PG_ISREADY, "-h", HOST, "-p", str(PORT)],
            capture_output=True,
            timeout=4,
            check=False,
        )
        code = result.returncode
    except FileNotFoundError:
        code = PG_MISCONFIGURED
    except subprocess.TimeoutExpired:
        code = PG_NO_RESPONSE
    duration = round(time.monotonic() - started, 3)

    if code == PG_ACCEPTING:
        status, summary = "ok", "Accepting connections"
    elif code == PG_REJECTING:
        status, summary = "critical", "Rejecting connections"
    elif code == PG_NO_RESPONSE:
        status, summary = "critical", "No response"
    else:
        status, summary = "unknown", "Check unavailable"
    print(
        json.dumps(
            {
                "schema_version": 1,
                "check_id": CHECK_ID,
                "status": status,
                "summary": summary[:200],
                "metrics": [
                    {
                        "metric_id": "check.duration_seconds",
                        "value": duration,
                        "unit": "seconds",
                    }
                ],
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
