//! chvctl library surface.
//!
//! The crate is split lib + bin so integration tests (the #372
//! contract-test harness, `tests/contract.rs`) can drive the CLI's real
//! request-building code — `commands::<group>::execute` over a
//! [`client::BffClient`] — against a live BFF router without spawning a
//! process. The binary (`src/main.rs`) is a thin wrapper over this lib;
//! its CLI surface and behavior are byte-identical to the pre-split
//! bin-only crate (kubedoio/chv#372 design §4 Option B).

pub mod client;
pub mod commands;
pub mod config;
pub mod output;
