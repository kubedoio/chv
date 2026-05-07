//! Stub (no-op) PolicyEnforcer for platforms without eBPF support.
//!
//! Re-exports [`NoopEbpfManager`] from the `ebpf` module. This module exists
//! as a named entry point for discoverability; the implementation lives in
//! `ebpf.rs` alongside the trait definition to keep everything co-located.

pub use crate::ebpf::NoopEbpfManager;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebpf::EbpfManager;

    #[test]
    fn noop_is_not_available() {
        let mgr = NoopEbpfManager;
        assert!(!mgr.is_available());
    }
}
