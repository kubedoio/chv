//! Seeder algorithm — `seed_if_first_deployment` and per-fixture `seed_one`.
//!
//! Hard rules (from the plan):
//!
//! - **Sentinel-gated**: read `system_settings.seed_starters_completed`. If
//!   it is already `'1'`, skip immediately and return `SeedOutcome::Skipped`.
//! - **Fail-open per fixture**: a single fixture that fails to parse,
//!   validate, or insert is logged via `tracing::error!` and skipped — it
//!   must not block the control plane from coming up.
//! - **Fail-closed on the sentinel**: the sentinel update at the end is
//!   propagated. A control plane that cannot read or update the sentinel
//!   refuses to start (it is a DB-health signal).
//! - **System-owned, draft status**: every seeded row has
//!   `owner_user_id = NULL` and `status = draft`. The dashboard never
//!   auto-applies a starter; the operator clones it first.

use chv_architecture_validate::{parse_yaml, run_static_checks};
use chv_controlplane_store::{TopologyCreateInput, TopologyRepository};
use chv_controlplane_types::architecture::{ArchitectureId, ArchitectureStatus, Severity};

use crate::error::SeedError;
use crate::starters::{StarterFixture, STARTER_FIXTURES};

/// Outcome of a [`seed_if_first_deployment`] call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedOutcome {
    /// Sentinel was already `'1'`; the seeder did nothing.
    Skipped,
    /// Seeder ran. `count` is the number of starters that landed in the
    /// store on this invocation (not the running total — operators who
    /// re-flip the sentinel will see only the ones that were actually
    /// inserted this run).
    Seeded { count: usize },
}

/// Pre-seed the six starter topologies on first deployment.
///
/// Idempotent on re-boot via the `seed_starters_completed` sentinel in
/// `system_settings`. See module docs for fail-open / fail-closed rules.
///
/// **Concurrency model:** the very first statement is an atomic
/// `UPDATE … WHERE value = '0'` that flips the sentinel to `'1'`. Only
/// the process whose UPDATE affects exactly one row holds the seed claim;
/// any other concurrently-booting process sees rows_affected = 0 and
/// returns [`SeedOutcome::Skipped`] without touching `architecture_topologies`.
/// This collapses the read-then-write race window to a single atomic
/// statement under SQLite's per-connection write lock — no two control
/// planes can both believe they are the seed claimant.
///
/// The atomic claim doubles as the fail-closed sentinel write: if the
/// row was deleted out-of-band, the UPSERT here re-creates it with
/// `value = '1'` so the seeder converges to the documented state instead
/// of silently looping forever on every boot.
pub async fn seed_if_first_deployment(repo: &TopologyRepository) -> Result<SeedOutcome, SeedError> {
    // Atomic claim. UPSERT semantics:
    // - If the row exists with value='0': flip to '1', rows_affected=1, we own the claim.
    // - If the row exists with value='1' or any other value: WHERE clause fails,
    //   no UPDATE; the INSERT … ON CONFLICT path also no-ops because the
    //   conflict is on the primary key. rows_affected=0 → skip.
    // - If the row was deleted out-of-band: INSERT lands with value='1',
    //   rows_affected=1. We took the claim AND restored the sentinel —
    //   correct on first principles, since the sentinel-missing case is
    //   indistinguishable from "first ever boot" from this layer's view.
    //
    // Trim-tolerant: a manual `UPDATE … SET value='1 '` (trailing whitespace
    // typo) does NOT match `value = '0'`, so it correctly stays in the
    // skipped state for the lifetime of that boot. Operators who *want*
    // to re-seed must set value to exactly '0'.
    let claimed = sqlx::query(
        "INSERT INTO system_settings (key, value, updated_at) \
         VALUES ('seed_starters_completed', '1', strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         ON CONFLICT(key) DO UPDATE \
             SET value = '1', \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
             WHERE trim(system_settings.value) = '0'",
    )
    .execute(repo.pool())
    .await?
    .rows_affected();

    if claimed == 0 {
        tracing::debug!("starter topologies already seeded; skipping");
        return Ok(SeedOutcome::Skipped);
    }

    let mut seeded = 0usize;
    for (index, fixture) in STARTER_FIXTURES.iter().enumerate() {
        match seed_one(repo, fixture, index).await {
            Ok(id) => {
                tracing::info!(
                    starter = %fixture.name,
                    id = %id,
                    "seeded starter topology"
                );
                seeded += 1;
            }
            Err(SeedError::AlreadyExists { starter }) => {
                // Reachable when a prior partial seed (e.g. process killed
                // mid-loop) left some rows behind, then someone manually
                // reset the sentinel. Per-fixture skip is correct.
                tracing::warn!(starter = %starter, "starter already exists; skipping");
            }
            Err(err) => {
                tracing::error!(
                    starter = %fixture.name,
                    error = %err,
                    "starter seed failed; continuing with remaining starters"
                );
            }
        }
    }

    tracing::info!(count = seeded, "starter topology seeding complete");
    Ok(SeedOutcome::Seeded { count: seeded })
}

/// Seed one starter fixture, returning its ID on success.
///
/// Performs the parse → static-check → insert pipeline:
///
/// 1. Parse YAML into a `CHVArchitecture`. Parse failures map to
///    [`SeedError::FixtureParse`].
/// 2. Run [`run_static_checks`]. ANY error-severity finding maps to
///    [`SeedError::FixtureValidation`] — the fixture is rejected, not
///    inserted.
/// 3. Serialize the model to JSON for `design_graph_json` so the UI's
///    canvas can render the starter without re-parsing.
/// 4. Insert the row with deterministic id `starter-NN-<slug>`,
///    `status = draft`, `owner_user_id = NULL`.
///
/// Index is 0-based; the on-the-wire id uses `index + 1` so operators see
/// `starter-01-single-vm` for the first one.
pub async fn seed_one(
    repo: &TopologyRepository,
    fixture: &StarterFixture,
    index: usize,
) -> Result<ArchitectureId, SeedError> {
    let model = parse_yaml(fixture.yaml).map_err(|err| SeedError::FixtureParse {
        starter: fixture.name.to_string(),
        message: err.to_string(),
    })?;

    let findings = run_static_checks(&model);
    let error_count = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    if error_count > 0 {
        return Err(SeedError::FixtureValidation {
            starter: fixture.name.to_string(),
            errors_count: error_count,
        });
    }

    // `serde_json::to_string` cannot fail for a `CHVArchitecture` whose
    // fields are all serde-derived plain data; treat a failure as a bug,
    // not a runtime condition.
    let design_graph_json =
        serde_json::to_string(&model).map_err(|err| SeedError::FixtureParse {
            starter: fixture.name.to_string(),
            message: format!("design_graph_json serialization failed: {err}"),
        })?;

    let id = ArchitectureId::new(format!("starter-{:02}-{}", index + 1, fixture.slug)).map_err(
        |err| SeedError::FixtureParse {
            starter: fixture.name.to_string(),
            message: format!("id construction failed: {err}"),
        },
    )?;

    let display_name = model.metadata.display_name.clone();
    let description = model.metadata.description.clone();

    let input = TopologyCreateInput {
        id: id.clone(),
        name: fixture.name.to_string(),
        display_name,
        description,
        environment: Some(fixture.environment.to_string()),
        status: ArchitectureStatus::Draft,
        owner_user_id: None,
        design_graph_json: Some(design_graph_json),
        latest_yaml: Some(fixture.yaml.to_string()),
    };

    match repo.create(input).await {
        Ok(_) => Ok(id),
        Err(chv_controlplane_store::StoreError::Conflict { .. }) => Err(SeedError::AlreadyExists {
            starter: fixture.name.to_string(),
        }),
        Err(err) => Err(SeedError::Store(err)),
    }
}
