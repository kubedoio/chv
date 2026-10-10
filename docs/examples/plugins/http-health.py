#!/usr/bin/env python3
"""Example chv-monitor plugin: local HTTP endpoint health check.

Part of the CHV native monitoring campaign (prompt 04, G4 part 1).
This is an EXAMPLE, not an installed component: the local root
administrator copies it into the plugin allowlist directory and pins
its digest in a manifest. See docs/examples/plugins/README.md.

The plugin checks one locally configured HTTP endpoint and prints a
plugin-output-v1 JSON object on stdout. It carries NO credentials,
never echoes the URL or any response body into its summary, and
reports exception class names only — a secret-bearing error page must
not leak into monitoring telemetry.
"""

import json
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import urlopen

# --- local administrator configuration (root-owned file) ---------------
# The manager never supplies or configures any of this. Edit as root,
# then recompute the manifest digest (see README).
URL = "http://127.0.0.1:8080/health"
TIMEOUT_SECONDS = 3.0
# ------------------------------------------------------------------------

CHECK_ID = "plugin:example.http-health"
EXPECTED_STATUS = 200


def main() -> int:
    started = time.monotonic()
    status = "ok"
    summary = "Endpoint responded"
    try:
        with urlopen(URL, timeout=TIMEOUT_SECONDS) as response:
            if response.status != EXPECTED_STATUS:
                status = "critical"
                summary = "HTTP status {}".format(response.status)
    except HTTPError as exc:
        # Non-2xx statuses raise before a response object is returned.
        status = "critical"
        summary = "HTTP status {}".format(exc.code)
    except URLError as exc:
        # Class name only: reason strings can embed URLs or file paths.
        status = "critical"
        summary = (
            type(exc.reason).__name__
            if isinstance(exc.reason, BaseException)
            else "Connection failed"
        )
    except Exception as exc:  # noqa: BLE001 - report the class, never details
        status = "critical"
        summary = type(exc).__name__
    duration = round(time.monotonic() - started, 3)
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
