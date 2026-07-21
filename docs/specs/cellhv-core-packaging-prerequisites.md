# CellHV Core Packaging Prerequisites

Status: shipped path prerequisites; production Core mode remains disabled.

The node package publishes explicit defaults for the future in-place
`chv-agent` Core authority:

- store: `/var/lib/chv/agent/core.db`;
- native API socket: `/run/chv/core/core-v1.sock`;
- persistent runtime lease: derived beside `core.db` and never removed by
  systemd, tmpfiles, package removal, or installer cleanup.

The package postinstall, tmpfiles definition, and systemd unit provision the
Core parents `/var/lib/chv/agent` and `/run/chv/core` as `chv:chv` 0700.
Core database and native socket creation enforce 0600 explicitly. The service
retains its legacy `UMask=002` so Cloud Hypervisor and existing child behavior
does not change. Both packaged and standalone units run as `User=chv`; systemd
`RuntimeDirectory`/`StateDirectory` provisioning avoids privileged ownership
changes from service precommands. The agent explicitly sets its existing API
socket mode after bind for legacy group clients, and `/run/chv/agent` remains
0775. Legacy shutdown may remove `api.sock` by pathname; this exception does
not apply to the separately rooted native Core socket.

Socket cleanup remains the listener owner's responsibility and is guarded by
its recorded device/inode. Units and install scripts do not blindly unlink the
native socket. These changes create no database, acquire no lease, start no
additional service, and do not enable production Core startup.
