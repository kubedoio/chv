//! NetBox projection runner — executes a projection plan against a live
//! NetBox instance (PR 4 of the #239 plan).
//!
//! The runner is the I/O shell around the pure core: it builds the
//! desired objects ([`crate::mapping::build_objects`]), fetches the
//! remote NetBox state (list-by-architecture for owned/stale candidates
//! plus natural-key lookups for foreign-occupancy detection), computes
//! the plan ([`crate::plan::compute_plan`]), and executes the entries
//! in plan order. All decisions stay in the pure core; the runner only
//! materializes them over HTTP.
//!
//! # Execution policy (fixed v1)
//!
//! - **Abort-on-first-hard-failure**: on the first mutation error the
//!   runner stops, records the failure plus every entry that was not
//!   attempted, and returns a partial outcome with `error: Some(...)`.
//!   The partial state is resumable — on re-run, `compute_plan` matches
//!   the already-written objects by external id and continues without
//!   duplicates.
//! - **Conflicts never write**: `conflict` entries are recorded and
//!   skipped, never sent to NetBox.
//! - **Delete is belt-and-braces**: under `delete` retention, a `stale`
//!   entry is only deleted after re-verifying the ownership marker on
//!   the remote object (on top of the pure plan's own guard).
//! - **Outcomes are secret-free**: error strings come from
//!   [`crate::client::ClientError`] displays, which never contain token
//!   material.

use chv_architecture_validate::fleet::InventorySnapshot;
use chv_architecture_validate::model::CHVArchitecture;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use tracing::debug;

use crate::client::{ClientError, NetBoxClient, RemoteNetBoxObject};
use crate::mapping::{
    build_objects, MappingError, MappingOutput, NetBoxKind, NetBoxObject, ProjectionConfigView,
    ProjectionInput as MappingProjectionInput,
};
use crate::ownership::{CustomFieldNames, ManagedMarker};
use crate::plan::{
    compute_plan, NetBoxRemoteObject, NetboxPlanAction, NetboxProjectionPlan,
    NetboxProjectionPlanEntry, PlanContext, PlanError, RetentionPolicy,
};

/// Everything the runner needs, store-free: the applied architecture
/// plus the projection config's pure slice. The custom-field prefix
/// lives inside `names` ([`CustomFieldNames::prefix`]) — a separate
/// `prefix` field would allow the two to diverge.
#[derive(Clone)]
pub struct NetboxProjectionInput<'a> {
    /// The applied, validated CHVArchitecture model (authoritative).
    pub architecture: &'a CHVArchitecture,
    /// Architecture id used in external ids and ownership fields.
    pub architecture_id: &'a str,
    /// Applied architecture version number (provenance).
    pub architecture_version: u64,
    /// Live fleet facts; `None` projects declared state only.
    pub snapshot: Option<&'a InventorySnapshot>,
    /// NetBox site label for projected devices.
    pub site_name: Option<&'a str>,
    /// Retention policy in effect for `stale` entries.
    pub retention: RetentionPolicy,
    /// Custom-field names (prefix-configurable) for marker parsing.
    pub names: CustomFieldNames,
}

/// Pre-execution failures (mapping, plan, or the remote-state fetch).
/// Mutation failures are NOT errors — they produce a partial
/// [`NetboxProjectionOutcome`] with `error: Some(..)` so the executed
/// work is reportable and the run resumable.
#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("mapping failed: {0}")]
    Mapping(#[from] MappingError),
    #[error("plan failed: {0}")]
    Plan(#[from] PlanError),
    #[error("netbox client error: {0}")]
    Client(#[from] ClientError),
}

impl RunnerError {
    /// `true` when the failure looks transient (transport, auth, or a
    /// server-side 5xx) and an automatic retry of the run is
    /// reasonable; mapping/plan failures are deterministic and never
    /// retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Mapping(_) | Self::Plan(_) => false,
            Self::Client(err) => err.is_transient(),
        }
    }
}

/// Per-entry execution status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetboxEntryStatus {
    Succeeded,
    Failed,
    /// Recorded without any write (`no_op` / `conflict` entries).
    Skipped,
    /// Not executed because an earlier entry hit a hard failure.
    NotAttempted,
}

impl NetboxEntryStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::NotAttempted => "not_attempted",
        }
    }
}

/// Outcome of executing one plan entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetboxEntryOutcome {
    pub action: NetboxPlanAction,
    pub kind: NetBoxKind,
    pub chv_resource_ref: String,
    pub status: NetboxEntryStatus,
    pub error: Option<String>,
}

/// Aggregate outcome counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetboxOutcomeSummary {
    pub succeeded: i64,
    pub failed: i64,
    pub skipped: i64,
    pub not_attempted: i64,
}

/// Run-level failure summary (abort-on-first-hard-failure).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetboxRunnerErrorSummary {
    /// Secret-free error message (client error display).
    pub message: String,
    /// The entry that failed, when the failure maps to one.
    pub failed_chv_resource_ref: Option<String>,
    /// Whether the failure class is transient (transport / auth /
    /// server 5xx) and the run may be retried automatically. Older
    /// persisted outcomes predate the field and default to `false`.
    #[serde(default)]
    pub retryable: bool,
}

/// The full export outcome: the plan, per-entry results, counts, and
/// the abort error when one occurred. Serde-serializable and
/// BTree-ordered throughout so persisted results are byte-stable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetboxProjectionOutcome {
    pub plan: NetboxProjectionPlan,
    pub entries: Vec<NetboxEntryOutcome>,
    pub summary: NetboxOutcomeSummary,
    pub error: Option<NetboxRunnerErrorSummary>,
}

/// Executes projection plans against NetBox.
pub struct NetboxProjectionRunner {
    client: NetBoxClient,
}

impl NetboxProjectionRunner {
    pub fn new(client: NetBoxClient) -> Self {
        Self { client }
    }

    /// Compute the plan only — no mutations. This is what dry-run runs
    /// execute.
    pub async fn dry_run(
        &self,
        input: &NetboxProjectionInput<'_>,
    ) -> Result<NetboxProjectionPlan, RunnerError> {
        let (_, plan, _) = self.prepare(input).await?;
        Ok(plan)
    }

    /// Build, fetch, plan, and execute. Returns `Err` only for
    /// pre-execution failures; mutation failures abort the loop and
    /// return a partial outcome with `error: Some(..)`.
    pub async fn run(
        &self,
        input: &NetboxProjectionInput<'_>,
    ) -> Result<NetboxProjectionOutcome, RunnerError> {
        debug!(
            architecture_id = input.architecture_id,
            architecture_version = input.architecture_version,
            "netbox projection execution starting"
        );
        let (desired, plan, remote_by_id) = self.prepare(input).await?;

        // Resolution indexes: (kind rank, natural key) and (kind rank,
        // raw external id) → NetBox id, mirroring the lookups
        // `compute_plan` performed, plus the id-keyed remote objects for
        // the delete-side ownership re-verification.
        let mut remote_by_key: BTreeMap<(u8, String), i64> = BTreeMap::new();
        let mut remote_by_ext: BTreeMap<(u8, String), i64> = BTreeMap::new();
        for (netbox_id, remote) in &remote_by_id {
            let key = (remote.kind.rank(), natural_key_string(&remote.natural_key));
            // `remote_by_id` iterates in ascending NetBox-id order, the
            // same order `compute_plan` saw — on a natural key occupied
            // by several objects, keep the first (lowest-id) match so
            // the runner resolves exactly the object the pure core
            // planned against.
            remote_by_key.entry(key).or_insert(*netbox_id);
            if let Some(ext) = remote.custom_fields.get(&input.names.external_id) {
                remote_by_ext.insert((remote.kind.rank(), ext.clone()), *netbox_id);
            }
        }

        let mut desired_by_key: BTreeMap<(u8, String), &NetBoxObject> = BTreeMap::new();
        for object in &desired.objects {
            desired_by_key.insert(
                (
                    object.kind().rank(),
                    natural_key_string(&object.natural_key()),
                ),
                object,
            );
        }

        let mut outcomes: Vec<NetboxEntryOutcome> = Vec::with_capacity(plan.entries.len());
        let mut summary = NetboxOutcomeSummary::default();
        let mut error: Option<NetboxRunnerErrorSummary> = None;
        // IP-address entries queued for the post-loop assignment fix-up
        // (a safety net: the contract's kind order creates interfaces
        // before their IP addresses, so the create-time interface
        // lookup normally resolves — the fix-up covers the cases it
        // cannot, e.g. an ambiguous match or a resumed run):
        // (outcome index, NetBox id, desired object).
        let mut ip_fixups: Vec<(usize, i64, &NetBoxObject)> = Vec::new();

        for entry in &plan.entries {
            if error.is_some() {
                // Abort-on-first-hard-failure: mutation entries after
                // the failure are recorded as not attempted; read-only
                // entries are still recorded (skipped) since they
                // involve no writes.
                let status = match entry.action {
                    NetboxPlanAction::Create
                    | NetboxPlanAction::Update
                    | NetboxPlanAction::Stale => NetboxEntryStatus::NotAttempted,
                    NetboxPlanAction::NoOp | NetboxPlanAction::Conflict => {
                        NetboxEntryStatus::Skipped
                    }
                };
                outcomes.push(entry_outcome(entry, status, None));
                continue;
            }

            let outcome = match entry.action {
                NetboxPlanAction::NoOp | NetboxPlanAction::Conflict => {
                    // Never write; conflicts are recorded, not executed.
                    entry_outcome(entry, NetboxEntryStatus::Skipped, None)
                }
                NetboxPlanAction::Create => {
                    let key = (
                        entry.kind.rank(),
                        natural_key_string(&entry.netbox_natural_key),
                    );
                    match desired_by_key.get(&key) {
                        None => entry_outcome(
                            entry,
                            NetboxEntryStatus::Failed,
                            Some("plan entry has no matching desired object".to_string()),
                        ),
                        Some(object) => match self.client.create_object(object).await {
                            Ok(netbox_id) => {
                                if matches!(object, NetBoxObject::IpAddress(_))
                                    && ip_needs_fixup(object)
                                {
                                    ip_fixups.push((outcomes.len(), netbox_id, object));
                                }
                                entry_outcome(entry, NetboxEntryStatus::Succeeded, None)
                            }
                            Err(err) => {
                                error = Some(runner_error(&err, entry));
                                entry_outcome(
                                    entry,
                                    NetboxEntryStatus::Failed,
                                    Some(err.to_string()),
                                )
                            }
                        },
                    }
                }
                NetboxPlanAction::Update => {
                    let key = (
                        entry.kind.rank(),
                        natural_key_string(&entry.netbox_natural_key),
                    );
                    // Primary resolution is the natural key; the
                    // fallback is the **external-id** index — the
                    // renamed-object case, where the plan's `update`
                    // refers to a chv-owned remote whose natural key no
                    // longer matches (the object was renamed in NetBox
                    // and the desired natural key is free). Both maps
                    // are keyed by (kind rank, …); mixing them up would
                    // make the fallback unmatchable.
                    let netbox_id = remote_by_key.get(&key).or_else(|| {
                        remote_by_ext.get(&(entry.kind.rank(), entry.external_id.clone()))
                    });
                    match (desired_by_key.get(&key), netbox_id) {
                        (Some(object), Some(&netbox_id)) => {
                            match self.client.update_object(netbox_id, object).await {
                                Ok(()) => {
                                    if matches!(object, NetBoxObject::IpAddress(_))
                                        && ip_needs_fixup(object)
                                    {
                                        ip_fixups.push((outcomes.len(), netbox_id, object));
                                    }
                                    entry_outcome(entry, NetboxEntryStatus::Succeeded, None)
                                }
                                Err(err) => {
                                    error = Some(runner_error(&err, entry));
                                    entry_outcome(
                                        entry,
                                        NetboxEntryStatus::Failed,
                                        Some(err.to_string()),
                                    )
                                }
                            }
                        }
                        // The plan proposed an update but the remote
                        // object cannot be resolved — inconsistent
                        // remote state; fail closed rather than guess.
                        _ => {
                            let message =
                                "plan proposed an update but the remote object cannot be resolved"
                                    .to_string();
                            error = Some(NetboxRunnerErrorSummary {
                                message: message.clone(),
                                failed_chv_resource_ref: Some(entry.chv_resource_ref.clone()),
                                retryable: false,
                            });
                            entry_outcome(entry, NetboxEntryStatus::Failed, Some(message))
                        }
                    }
                }
                NetboxPlanAction::Stale => {
                    let ext_key = (entry.kind.rank(), entry.external_id.clone());
                    let key = (
                        entry.kind.rank(),
                        natural_key_string(&entry.netbox_natural_key),
                    );
                    let netbox_id = remote_by_ext
                        .get(&ext_key)
                        .or_else(|| remote_by_key.get(&key));
                    match netbox_id {
                        None => {
                            let message = "stale entry has no resolvable remote object".to_string();
                            error = Some(NetboxRunnerErrorSummary {
                                message: message.clone(),
                                failed_chv_resource_ref: Some(entry.chv_resource_ref.clone()),
                                retryable: false,
                            });
                            entry_outcome(entry, NetboxEntryStatus::Failed, Some(message))
                        }
                        Some(&netbox_id) => match input.retention {
                            RetentionPolicy::MarkStale => {
                                match self
                                    .client
                                    .mark_stale(netbox_id, entry.kind, &input.names.managed_state)
                                    .await
                                {
                                    Ok(()) => {
                                        entry_outcome(entry, NetboxEntryStatus::Succeeded, None)
                                    }
                                    Err(err) => {
                                        error = Some(runner_error(&err, entry));
                                        entry_outcome(
                                            entry,
                                            NetboxEntryStatus::Failed,
                                            Some(err.to_string()),
                                        )
                                    }
                                }
                            }
                            RetentionPolicy::Delete => {
                                // Belt-and-braces on top of the pure
                                // plan: only delete an object whose
                                // remote marker still proves our
                                // ownership of this architecture.
                                let owned = remote_by_id.get(&netbox_id).and_then(|remote| {
                                    ManagedMarker::parse(&remote.custom_fields, &input.names)
                                });
                                let verified = owned
                                    .filter(|marker| {
                                        marker.is_owned_by_chv()
                                            && marker.architecture_id == input.architecture_id
                                    })
                                    .is_some();
                                if !verified {
                                    entry_outcome(
                                        entry,
                                        NetboxEntryStatus::Skipped,
                                        Some(
                                            "ownership re-verification failed; object not deleted"
                                                .to_string(),
                                        ),
                                    )
                                } else {
                                    match self.client.delete_object(netbox_id, entry.kind).await {
                                        Ok(()) => {
                                            entry_outcome(entry, NetboxEntryStatus::Succeeded, None)
                                        }
                                        Err(err) => {
                                            error = Some(runner_error(&err, entry));
                                            entry_outcome(
                                                entry,
                                                NetboxEntryStatus::Failed,
                                                Some(err.to_string()),
                                            )
                                        }
                                    }
                                }
                            }
                        },
                    }
                }
            };
            outcomes.push(outcome);
        }

        // Post-loop assignment fix-up: the contract's kind order
        // creates interfaces before their IP addresses, so the
        // create-time body builder normally resolves the interface id
        // in-loop. The fix-up re-applies the write for the cases it
        // cannot — an interface that did not exist or matched
        // ambiguously at create time (e.g. a resumed run). Only runs
        // when nothing aborted.
        if error.is_none() {
            for (outcome_index, netbox_id, object) in ip_fixups {
                if let Err(err) = self.client.update_object(netbox_id, object).await {
                    error = Some(NetboxRunnerErrorSummary {
                        message: err.to_string(),
                        failed_chv_resource_ref: Some(
                            outcomes[outcome_index].chv_resource_ref.clone(),
                        ),
                        retryable: err.is_transient(),
                    });
                    outcomes[outcome_index].status = NetboxEntryStatus::Failed;
                    outcomes[outcome_index].error = Some(err.to_string());
                    break;
                }
            }
        }

        for outcome in &outcomes {
            match outcome.status {
                NetboxEntryStatus::Succeeded => summary.succeeded += 1,
                NetboxEntryStatus::Failed => summary.failed += 1,
                NetboxEntryStatus::Skipped => summary.skipped += 1,
                NetboxEntryStatus::NotAttempted => summary.not_attempted += 1,
            }
        }

        debug!(
            succeeded = summary.succeeded,
            failed = summary.failed,
            skipped = summary.skipped,
            not_attempted = summary.not_attempted,
            aborted = error.is_some(),
            "netbox projection execution finished"
        );

        Ok(NetboxProjectionOutcome {
            plan,
            entries: outcomes,
            summary,
            error,
        })
    }

    /// Steps 1–3 shared by `run` and `dry_run`: build the desired
    /// objects, fetch the remote state view, compute the plan. The
    /// remote state is returned id-keyed so `run` can resolve plan
    /// entries back to NetBox ids.
    async fn prepare(
        &self,
        input: &NetboxProjectionInput<'_>,
    ) -> Result<
        (
            MappingOutput,
            NetboxProjectionPlan,
            BTreeMap<i64, NetBoxRemoteObject>,
        ),
        RunnerError,
    > {
        let desired = build_objects(&MappingProjectionInput {
            architecture: input.architecture,
            architecture_id: input.architecture_id,
            architecture_version: input.architecture_version,
            snapshot: input.snapshot,
            config: ProjectionConfigView {
                custom_field_prefix: input.names.prefix.clone(),
                site_name: input.site_name.map(str::to_string),
            },
        })?;
        let remote_by_id = self
            .fetch_remote_state(&desired.objects, &input.names, input.architecture_id)
            .await?;
        let remote: Vec<NetBoxRemoteObject> = remote_by_id.values().cloned().collect();
        let plan = compute_plan(
            &desired,
            &remote,
            &PlanContext {
                architecture_id: input.architecture_id.to_string(),
                architecture_version: input.architecture_version,
                names: input.names.clone(),
                retention: input.retention,
            },
        )?;
        debug!(
            create = plan.summary.create,
            update = plan.summary.update,
            no_op = plan.summary.no_op,
            conflict = plan.summary.conflict,
            stale = plan.summary.stale,
            "netbox projection plan computed"
        );
        Ok((desired, plan, remote_by_id))
    }

    /// Fetch the remote state view: per kind, all objects of this
    /// architecture (owned + stale candidates, via the custom-field
    /// filter) **plus** a natural-key probe for every desired object
    /// (foreign-occupancy detection — a foreign object squatting on a
    /// desired natural key is invisible to the architecture filter but
    /// must produce a `conflict`, never a doomed create).
    ///
    /// The six per-kind lists are fetched **concurrently** (each is a
    /// full paginated query that can take up to the request timeout);
    /// the natural-key probes stay sequential — they are per-object and
    /// bounded by the plan size. Results are deduplicated by NetBox id
    /// into a `BTreeMap`, so the order handed to `compute_plan` stays
    /// deterministic regardless of completion order.
    ///
    /// Natural-key ambiguity (a probe matching several remote objects,
    /// e.g. `10.42.0.5/24` and `10.42.0.5/32`) is **not** an error:
    /// every match enters the remote state, and the pure core degrades
    /// the collision to a per-entry `conflict` — the run continues.
    async fn fetch_remote_state(
        &self,
        desired: &[NetBoxObject],
        names: &CustomFieldNames,
        architecture_id: &str,
    ) -> Result<BTreeMap<i64, NetBoxRemoteObject>, ClientError> {
        let mut by_id: BTreeMap<i64, NetBoxRemoteObject> = BTreeMap::new();

        // The six list methods have distinct opaque future types, so
        // they are boxed to share one `try_join_all` input.
        let kind_lists: Vec<
            futures_util::future::BoxFuture<'_, Result<Vec<RemoteNetBoxObject>, ClientError>>,
        > = vec![
            Box::pin(
                self.client
                    .list_vlans_by_architecture(&names.architecture_id, architecture_id),
            ),
            Box::pin(
                self.client
                    .list_prefixes_by_architecture(&names.architecture_id, architecture_id),
            ),
            Box::pin(
                self.client
                    .list_ip_addresses_by_architecture(&names.architecture_id, architecture_id),
            ),
            Box::pin(
                self.client
                    .list_interfaces_by_architecture(&names.architecture_id, architecture_id),
            ),
            Box::pin(
                self.client
                    .list_virtual_machines_by_architecture(&names.architecture_id, architecture_id),
            ),
            Box::pin(
                self.client
                    .list_devices_by_architecture(&names.architecture_id, architecture_id),
            ),
        ];
        for list in futures_util::future::try_join_all(kind_lists).await? {
            for entry in list {
                by_id.insert(entry.netbox_id, entry.object);
            }
        }

        for object in desired {
            let found = match object {
                NetBoxObject::Device(d) => self.client.get_devices_by_name(&d.name).await?,
                NetBoxObject::VirtualMachine(v) => {
                    self.client.get_virtual_machines_by_name(&v.name).await?
                }
                NetBoxObject::Interface(i) => {
                    self.client
                        .get_interfaces_by_name(&i.name, &i.virtual_machine)
                        .await?
                }
                NetBoxObject::Prefix(p) => self.client.get_prefixes_by_cidr(&p.prefix).await?,
                NetBoxObject::Vlan(v) => self.client.get_vlans_by_vid(v.vid).await?,
                NetBoxObject::IpAddress(a) => {
                    self.client.get_ip_addresses_by_address(&a.address).await?
                }
            };
            for entry in found {
                by_id.insert(entry.netbox_id, entry.object);
            }
        }

        Ok(by_id)
    }
}

/// Deterministic string form of a natural key (BTreeMap order), used
/// for the runner's resolution indexes.
fn natural_key_string(natural_key: &BTreeMap<String, String>) -> String {
    natural_key
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";")
}

/// `true` when the IP address desires an interface assignment (and so
/// may need the post-loop fix-up).
fn ip_needs_fixup(object: &NetBoxObject) -> bool {
    matches!(object, NetBoxObject::IpAddress(a) if a.assigned_to_interface.is_some())
}

fn runner_error(err: &ClientError, entry: &NetboxProjectionPlanEntry) -> NetboxRunnerErrorSummary {
    NetboxRunnerErrorSummary {
        message: err.to_string(),
        failed_chv_resource_ref: Some(entry.chv_resource_ref.clone()),
        retryable: err.is_transient(),
    }
}

fn entry_outcome(
    entry: &NetboxProjectionPlanEntry,
    status: NetboxEntryStatus,
    error: Option<String>,
) -> NetboxEntryOutcome {
    NetboxEntryOutcome {
        action: entry.action,
        kind: entry.kind,
        chv_resource_ref: entry.chv_resource_ref.clone(),
        status,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_serializes_with_snake_case_statuses() {
        let outcome = NetboxEntryOutcome {
            action: NetboxPlanAction::Create,
            kind: NetBoxKind::Device,
            chv_resource_ref: "servers/chv-node-01".to_string(),
            status: NetboxEntryStatus::NotAttempted,
            error: None,
        };
        let json = serde_json::to_string(&outcome).expect("serializable");
        assert!(json.contains("\"not_attempted\""));
        assert!(json.contains("\"create\""));
        assert!(json.contains("\"device\""));
    }

    #[test]
    fn natural_key_string_is_deterministic() {
        let mut key = BTreeMap::new();
        key.insert("name".to_string(), "backend".to_string());
        key.insert("virtual_machine".to_string(), "vm-01".to_string());
        assert_eq!(
            natural_key_string(&key),
            "name=backend;virtual_machine=vm-01"
        );
    }
}
