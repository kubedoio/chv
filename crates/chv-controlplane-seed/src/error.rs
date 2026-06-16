//! Error type for the starter-topology seeder.

/// Errors produced by the starter-topology seeder.
///
/// `AlreadyExists` is per-fixture and non-fatal; the seeder logs and
/// continues. Every other variant is treated as a per-fixture failure
/// (logged, continued) by [`crate::starters::seed_if_first_deployment`],
/// except `Sqlx` errors raised while updating the sentinel row, which are
/// fatal — a control plane that cannot read or update the sentinel must not
/// finish booting (it is a DB-health signal).
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    #[error("starter {starter} already exists")]
    AlreadyExists { starter: String },

    #[error("fixture {starter} parse failed: {message}")]
    FixtureParse { starter: String, message: String },

    #[error("fixture {starter} validation failed: {errors_count} errors")]
    FixtureValidation {
        starter: String,
        errors_count: usize,
    },

    #[error("store error: {0}")]
    Store(#[from] chv_controlplane_store::StoreError),

    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
}
