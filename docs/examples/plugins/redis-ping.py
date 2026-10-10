#!/usr/bin/env python3
"""Example chv-monitor plugin: Redis PING check.

Part of the CHV native monitoring campaign (prompt 04, G4 part 1).
This is an EXAMPLE, not an installed component: the local root
administrator copies it into the plugin allowlist directory and pins
its digest in a manifest. See docs/examples/plugins/README.md.

The plugin speaks the minimal Redis RESP handshake (`PING`) over a
plain TCP socket and reports whether the server answered `PONG`. It
uses NO credentials: if the instance requires authentication the
server still answers (with an error), which is reported as a warning
— the service is up, the check simply cannot go deeper without a
secret, and secrets never belong in monitoring telemetry.
"""

import json
import socket
import sys
import time

# --- local administrator configuration (root-owned file) ---------------
HOST = "127.0.0.1"
PORT = 6379
TIMEOUT_SECONDS = 3.0
# ------------------------------------------------------------------------

CHECK_ID = "example.redis-ping"


def main() -> int:
    started = time.monotonic()
    status = "ok"
    summary = "PONG"
    try:
        with socket.create_connection((HOST, PORT), timeout=TIMEOUT_SECONDS) as sock:
            sock.settimeout(TIMEOUT_SECONDS)
            sock.sendall(b"PING\r\n")
            reply = sock.recv(64)
        if reply.startswith(b"+PONG"):
            status, summary = "ok", "PONG"
        elif reply.startswith(b"-"):
            # e.g. -NOAUTH or -ERR: the server answered, the check
            # refuses to carry credentials to go further.
            status, summary = "warning", "Server responded without PONG"
        else:
            status, summary = "warning", "Unexpected reply"
    except (ConnectionRefusedError, socket.timeout, OSError):
        status, summary = "critical", "No response"
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
