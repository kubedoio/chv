//! Pre-seeds CHV deployments with the six canonical starter topologies on
//! first boot, so an operator landing on `/architectures` sees a populated
//! dashboard instead of an empty state.
//!
//! Architecture overview (see `docs/plans/2026-06-16-starter-topologies-and-auto-seed.md`):
//!
//! - Starter YAML fixtures are embedded via `include_str!` and validated
//!   through [`chv_architecture_validate`] at seed time — fixtures that fail
//!   static checks are skipped (fail-open per starter).
//! - A sentinel row in `system_settings.seed_starters_completed` tracks
//!   whether seeding has happened for this deployment. Operators can opt out
//!   by flipping the row to `'1'` before first boot, or re-seed missing
//!   starters by flipping it back to `'0'` and restarting.
//! - The seeder is wire-up free at this layer — bootstrap.rs (Stage B)
//!   calls [`starters::seed_if_first_deployment`] after migrations succeed.

pub mod error;
pub mod starters;

pub use error::SeedError;
pub use starters::{seed_if_first_deployment, SeedOutcome, StarterFixture, STARTER_FIXTURES};

#[cfg(test)]
mod tests;
