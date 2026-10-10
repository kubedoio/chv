//! `chv-monitor-agent` library: the optional in-guest monitoring
//! agent (ADR-026). Read-only /proc collectors, one-shot claim
//! enrollment, mutual-TLS ingestion, bounded disk spool, credential
//! rotation. See
//! `docs/specs/contracts/chv-monitor-agent-security-plugins-v1.md`.

pub mod agent;
pub mod client;
pub mod config;
pub mod credential;
pub mod spool;
pub mod state;
pub mod wire;

pub use agent::{Agent, AgentError, TickOutcome};
pub use config::AgentConfig;
pub use credential::StoredCredential;
