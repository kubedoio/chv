# CellHV OpenStack Discovery Lab

This lab supports Phase A2 discovery only. It does not qualify or claim
OpenStack, libvirt, network, or storage compatibility.

## Safety boundary

Use a dedicated, disposable host or VM which contains no production data or
credentials. The tooling reserves names beginning with `cellhv-osd-` and
refuses to run unless all of these conditions are true:

- `/etc/cellhv-test-host` contains exactly
  `cellhv-openstack-discovery-disposable-v1`;
- `CELLHV_LAB_CREDENTIAL_CLASS=disposable`;
- `OS_PROJECT_NAME` begins with `cellhv-osd-`;
- `OS_AUTH_URL` points to loopback, RFC 1918 IPv4, or a `.test` domain;
- immutable source revisions and image digests are recorded in the lab input
  file;
- common production credential variables are absent.

Creating the host marker is an infrastructure-owner action. Neither preflight
nor collection creates or changes it.

These checks are guardrails, not proof that credentials or infrastructure are
non-production. The lab owner remains responsible for verifying isolation.
`CELLHV_TEST_HOST_MARKER` is accepted only with
`CELLHV_PREFLIGHT_TEST_MODE=1`, for automated tests; never set that mode in a
lab run.

## Pinned inputs

Copy `lab-inputs.env.example` outside the repository evidence directory, fill
the unresolved immutable values, and keep the key names unchanged. A branch,
tag, package channel, or `latest` image is not a pin. Git inputs require a
40-character commit ID and binary/image inputs require a SHA-256 digest.

The proposed baseline is Ubuntu Server 24.04 on x86_64, Cloud Hypervisor
v43.0, and the OpenStack 2025.1 stable series. These are discovery inputs, not
a support matrix. The exact kernel, Nova, libvirt, firmware, and guest image
builds observed in the lab must also appear in the collected gap report.

```bash
set -a
source /secure/cellhv-osd/lab-inputs.env
source /secure/cellhv-osd/openrc
set +a
scripts/openstack-discovery/preflight.sh /secure/cellhv-osd/lab-inputs.env
```

Preflight is read-only. It performs no installation, checkout, service action,
network change, or credential validation request.

## Lab execution

Provision DevStack or an equivalent standalone deployment using the pinned
revisions. Keep its configuration and generated resources under the reserved
prefix. Enable only Nova compute, Placement, and minimal Neutron for the first
probe. Add Cinder only after compute discovery and record it independently.

For each candidate, stop at the first meaningful failure and record:

- exact component versions and configuration;
- first success and first failure;
- libvirt API calls and generated XML where available;
- exact Nova/libvirt source revision, file, symbol, and line references;
- QEMU-specific assumptions without representing Cloud Hypervisor as QEMU;
- network and storage expectations as separate findings;
- impact on `chv-agent` authority;
- security, maintenance, and test burden.

Do not patch through a chain of failures for a demonstration. Do not connect a
platform component directly to the Core database, provider internals, or Cloud
Hypervisor API sockets.

### Reproducible DevStack procedure

Start from a clean checkout owned by the disposable lab user. The commit IDs
come from the validated input file; the commands deliberately detach at those
immutable revisions.

```bash
git clone https://opendev.org/openstack/devstack.git /opt/stack/devstack
git -C /opt/stack/devstack checkout --detach "$CELLHV_DEVSTACK_COMMIT"
git clone https://opendev.org/openstack/nova.git /opt/stack/nova
git -C /opt/stack/nova checkout --detach "$CELLHV_NOVA_COMMIT"
install -m 0600 docs/labs/openstack-discovery/local.conf.example \
  /opt/stack/devstack/local.conf
cd /opt/stack/devstack
./stack.sh 2>&1 | tee /var/tmp/cellhv-osd-stack.log
```

Replace every `CHANGE_ME` password in the installed copy only. Never collect
that file. The template enables Nova compute/API/scheduler/conductor,
Placement, and minimal Neutron services; it disables Cinder initially. Its
`connection_uri = ch:///system` is a discovery input only and does not imply
that Nova accepts or supports the backend.

Run observe-only initial probes and preserve their exit status:

```bash
systemctl status devstack@n-cpu.service --no-pager \
  > /var/tmp/cellhv-osd-nova-compute-status.txt 2>&1
virsh -c ch:///system uri \
  > /var/tmp/cellhv-osd-libvirt-connection.txt 2>&1
virsh -c ch:///system capabilities \
  > /var/tmp/cellhv-osd-libvirt-capabilities.xml 2>&1
openstack compute service list \
  > /var/tmp/cellhv-osd-compute-services.txt 2>&1
```

Capture `/var/log/nova/nova-compute.log`, the effective redacted Nova
configuration, and any generated domain XML using the evidence allowlist.
Record the first failing operation before attempting any generic patch. Keep
Cinder disabled until compute, Neutron, and storage expectations have been
reported separately.

Teardown uses DevStack's scoped procedure, followed by the read-only verifier:

```bash
cd /opt/stack/devstack
./unstack.sh 2>&1 | tee /var/tmp/cellhv-osd-unstack.log
scripts/openstack-discovery/verify-cleanup.sh
```

## Evidence collection

Create a tab-separated allowlist. The first column is a descriptive evidence
kind and the second is an absolute path to one regular text file:

```text
nova-config\t/etc/nova/nova.conf
nova-log\t/var/log/nova/nova-compute.log
domain-xml\t/tmp/cellhv-osd-domain.xml
gap-report\t/tmp/cellhv-osd-gap-report.yaml
```

Collect into a new directory. Existing destinations are rejected.

```bash
scripts/openstack-discovery/collect.sh \
  /secure/cellhv-osd/lab-inputs.env \
  /secure/cellhv-osd/evidence-files.tsv \
  docs/evidence/openstack-discovery/20260721T120000Z-path-a
```

The collector copies only allowlisted regular files, refuses symlinks and
credential-like filenames, redacts common secret assignments and authorization
headers, writes a source-to-output index, and generates `SHA256SUMS`. Review
every collected file manually before publication; automated redaction is a
backstop, not a secrecy guarantee. Never collect OpenRC files, private keys,
cloud credential files, tokens, cookies, database dumps, or production data.

## Cleanup and verification

Use the lab provisioner's normal destroy operation. Remove only resources
created by that disposable deployment; do not use broad `pkill`, firewall
flushes, wildcard volume deletion, or unscoped network deletion. Preserve the
redacted evidence directory before destroying the host.

After teardown, run the read-only verifier:

```bash
scripts/openstack-discovery/verify-cleanup.sh
```

It fails if processes, interfaces, or network namespaces bearing the reserved
prefix remain. Provider-specific resources must also be checked using the
provider inventory captured for the run. Record the teardown command, its exit
status, the verifier output, and any residual resources in the gap report.

## Evidence integrity

Verify a transferred evidence directory with:

```bash
(cd docs/evidence/openstack-discovery/20260721T120000Z-path-a && sha256sum -c SHA256SUMS)
```

Checksums prove only that collected files did not change. They do not prove the
discovery result, infrastructure compatibility, or qualification.

## Bounded Path A runner

After provisioning the pinned lab, the fail-closed runner verifies the
effective `ch:///system` configuration, source revisions, Cloud Hypervisor
digest, package versions, and bounded command outcomes. The explicit flag
acknowledges that the probe restarts `nova-compute`:

```bash
scripts/openstack-discovery/run-path-a.py \
  --execute /secure/cellhv-osd/lab-inputs.env \
  /secure/cellhv-osd/path-a-run-id
```

The private output directory contains an `execution-manifest.json` with
ordered commands, exit statuses, timestamps, and artifact digests. Command
output is redacted before it is written; collection redacts it again. The
runner snapshots OpenStack resources, records the initial Nova compute service
state, restores that state in `finally`, compares the after-run inventory, and
runs the host cleanup verifier. Any missing restoration or cleanup result
keeps the probe blocked. Add every file to an explicit `collect.sh` allowlist
and inspect the redacted result before publication. A failed connection is
valid discovery evidence and must not be patched through into a demonstration.

Runner output is an unsigned `structural-candidate`, not T5 proof. The report
validator deliberately makes `complete` impossible from this unsigned
manifest. A separate trusted lab-attestation design and trust root must be
accepted before real-lab output can satisfy that gate.

`CELLHV_PREFLIGHT_TEST_MODE=1` permits path overrides only for unit fixtures.
Fixture manifests are permanently labeled `fixture`; the report validator
rejects them as complete T5 evidence.
