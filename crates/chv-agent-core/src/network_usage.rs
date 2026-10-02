//! Durable network-usage lookup for last-detach teardown (#356 N5).
//!
//! The Core runtime tears a network's host topology down (bridge /
//! namespace / dnsmasq / nft table via nwd) when the last VM on the node
//! that uses it is deleted. That decision must come from an authority that
//! survives daemon restarts — the Core store's VM definitions — because the
//! runtime's in-memory side-effect map is empty after a restart, and
//! "nothing references the network" would falsely authorize tearing down a
//! network a still-running pre-restart VM uses. Deleting the bridge is
//! unconditional in nwd's local teardown, so a wrong decision cuts that
//! VM's guest off the network.
//!
//! This reads the store through a READ-ONLY connection
//! ([`cellhv_core_store::CoreStore::open_read_only`]) held next to the
//! executor's read-write authority: WAL mode lets the short `list_vms`
//! reads coexist with the single-writer poller's transactions.

use std::path::Path;
use std::sync::Mutex;

use chv_hypervisor_api::NetworkUsageLookup;
use tracing::warn;

/// [`NetworkUsageLookup`] backed by a read-only Core store handle.
pub struct CoreStoreNetworkUsage {
    store: Mutex<cellhv_core_store::CoreStore>,
}

impl CoreStoreNetworkUsage {
    /// Open a read-only handle to the Core store at `path`.
    ///
    /// Errors when the file is absent, foreign, or unreadable — the caller
    /// then wires a fail-closed lookup instead (never a fail-open one).
    pub fn open(path: &Path) -> Result<Self, cellhv_core_store::StoreError> {
        Ok(Self {
            store: Mutex::new(cellhv_core_store::CoreStore::open_read_only(path)?),
        })
    }
}

impl NetworkUsageLookup for CoreStoreNetworkUsage {
    fn network_in_use(&self, network_id: &str, excluding_vm: &str) -> bool {
        // Fail CLOSED on any read error: a skipped teardown leaves
        // observable residue; a wrong teardown causes an outage. A poisoned
        // mutex likewise reports in-use instead of panicking — this runs on
        // a blocking-pool thread whose panic would take the executor down.
        let vms = match self.store.lock() {
            Ok(store) => match store.list_vms() {
                Ok(vms) => vms,
                Err(error) => {
                    warn!(
                        network_id,
                        excluding_vm,
                        %error,
                        "network-usage lookup failed; treating the network as in use (fail closed)"
                    );
                    return true;
                }
            },
            Err(poisoned) => {
                warn!(
                    network_id,
                    excluding_vm,
                    error = %poisoned,
                    "network-usage store lock poisoned; treating the network as in use (fail closed)"
                );
                return true;
            }
        };
        // Tombstone ordering (verified in cellhv-core-store): the store
        // applies a DeleteVm's tombstone when the operation is ACCEPTED —
        // BEFORE the effector runs — so the deleting VM is already absent
        // from `list_vms()` here and `excluding_vm` is belt-and-braces.
        // Correctness of last-detach therefore DEPENDS on tombstone-at-
        // accept: if the ordering ever changed to tombstone-at-finish, the
        // deleting VM would be listed during its own drain and teardown
        // would silently stop firing (fail-closed degradation, no outage).
        // The flip side — a VM whose delete was accepted and then FAILED
        // is also absent (tombstoned) while its VMM may still run — is
        // why the runtime gates teardown on the delete's success and adds
        // an in-session retained-entry veto; see core_runtime.rs.
        vms.iter().any(|vm| {
            vm.id.as_str() != excluding_vm
                && vm
                    .networks
                    .iter()
                    .any(|attachment| attachment.network_ref == network_id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cellhv_core_operations::{MutationCommand, OperationService, SubmitMutation};
    use cellhv_core_types::{
        HostId, HostIdentity, IdempotencyKey, OperationId, OperationRequestMetadata,
        ResourceVersion,
    };
    use std::os::unix::fs::PermissionsExt;

    fn service_at(path: &std::path::Path) -> OperationService {
        OperationService::create_new(
            path,
            &HostIdentity {
                id: HostId::new("node-test").unwrap(),
                resource_version: ResourceVersion::new(1).unwrap(),
            },
        )
        .unwrap()
    }

    fn definition(vm_id: &str, networks: &[&str]) -> cellhv_core_types::VmDefinition {
        cellhv_core_types::VmDefinition {
            id: cellhv_core_types::VmId::new(vm_id.to_owned()).unwrap(),
            name: vm_id.to_owned(),
            boot: cellhv_core_types::BootSpec {
                kernel: "kernel".to_owned(),
                firmware: None,
                initial_disk: None,
            },
            compute: cellhv_core_types::ComputeSpec::new(1, 1024).unwrap(),
            storage: vec![],
            networks: networks
                .iter()
                .map(|network| cellhv_core_types::NetworkAttachmentRef {
                    attachment_id: format!("{vm_id}-nic-{}", network),
                    network_ref: (*network).to_owned(),
                    mac_address: None,
                    addressing: None,
                    firewall_policy_json: None,
                })
                .collect(),
            requested_power_state: cellhv_core_types::RequestedPowerState::Stopped,
            observed_power_state: cellhv_core_types::ObservedPowerState::Unknown,
            resource_version: cellhv_core_types::ResourceVersion::new(1).unwrap(),
            cloud_init_userdata: None,
            hypervisor_tuning: None,
        }
    }

    fn submit_create(service: &mut OperationService, vm_id: &str, networks: &[&str]) {
        service
            .submit(SubmitMutation {
                operation_id: OperationId::new(format!("op-create-{vm_id}")).unwrap(),
                idempotency_scope: "test".to_owned(),
                idempotency_key: IdempotencyKey::new(format!("key-{vm_id}")).unwrap(),
                expected_vm_version: ResourceVersion::new(1).unwrap(),
                metadata: OperationRequestMetadata {
                    requested_by: "test".to_owned(),
                    external_operation_id: "test".to_owned(),
                    request_unix_ms: 1_700_000_000_000,
                    legacy_generation: None,
                },
                command: MutationCommand::CreateVm {
                    definition: definition(vm_id, networks),
                },
            })
            .unwrap();
    }

    #[test]
    fn reports_usage_from_the_authoritative_vm_list() {
        let dir = tempfile::tempdir().unwrap();
        // Normalize to 0700 regardless of umask: the Core fresh-parent
        // check rejects group/other-writable parents (same as the
        // projection tests).
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("core.db");
        let mut service = service_at(&path);
        submit_create(&mut service, "vm-a", &["net-1"]);
        submit_create(&mut service, "vm-b", &["net-1", "net-2"]);
        drop(service);

        let lookup = CoreStoreNetworkUsage::open(&path).unwrap();
        // Both live VMs use net-1 — in use no matter whom you exclude.
        assert!(lookup.network_in_use("net-1", "vm-a"));
        assert!(lookup.network_in_use("net-1", "vm-b"));
        // net-2 is only used by vm-b: excluding vm-b frees it, excluding
        // vm-a does not.
        assert!(lookup.network_in_use("net-2", "vm-a"));
        assert!(!lookup.network_in_use("net-2", "vm-b"));
        // A network nobody references.
        assert!(!lookup.network_in_use("net-unused", "vm-a"));
    }

    #[test]
    fn tombstoned_vms_do_not_protect_a_network() {
        // The store tombstones a VM when its DeleteVm is ACCEPTED — before
        // the effector runs. The lookup therefore treats a tombstoned VM
        // as gone (authoritative delete intent), which is why the runtime
        // gates teardown on the delete's SUCCESS, not on this lookup.
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("core.db");
        let mut service = service_at(&path);
        submit_create(&mut service, "vm-a", &["net-1"]);
        // Accept a delete for vm-a (tombstone at accept time).
        service
            .submit(SubmitMutation {
                operation_id: OperationId::new("op-delete-vm-a").unwrap(),
                idempotency_scope: "test".to_owned(),
                idempotency_key: IdempotencyKey::new("key-delete-vm-a").unwrap(),
                expected_vm_version: ResourceVersion::new(1).unwrap(),
                metadata: OperationRequestMetadata {
                    requested_by: "test".to_owned(),
                    external_operation_id: "test".to_owned(),
                    request_unix_ms: 1_700_000_000_000,
                    legacy_generation: None,
                },
                command: MutationCommand::DeleteVm {
                    vm_id: cellhv_core_types::VmId::new("vm-a".to_owned()).unwrap(),
                },
            })
            .unwrap();
        drop(service);

        let lookup = CoreStoreNetworkUsage::open(&path).unwrap();
        assert!(
            !lookup.network_in_use("net-1", "vm-a"),
            "a tombstoned VM must not keep its network marked in use"
        );
    }

    #[test]
    fn construction_fails_on_a_foreign_file_instead_of_opening_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-store.db");
        std::fs::write(&path, b"definitely not sqlite").unwrap();
        assert!(CoreStoreNetworkUsage::open(&path).is_err());
    }
}
