//! Linux-specific eBPF policy enforcer using TC commands.
//!
//! Re-exports [`LinuxEbpfManager`] from the `ebpf` module. On Linux with the
//! `ebpf` feature enabled, this implementation uses `tc` commands to attach
//! and detach BPF classifier programs. BPF map operations will use `libbpf-rs`
//! when the feature is active; without it, map updates are logged but not
//! applied (the TC program still runs if pre-loaded).
//!
//! This module is always compilable (the underlying `LinuxEbpfManager` uses
//! only `tokio::process::Command` for `tc` invocations). The `ebpf` feature
//! gates the `libbpf-rs` dependency for direct map manipulation.

pub use crate::ebpf::LinuxEbpfManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_ebpf_manager_not_available_without_file() {
        let mgr = LinuxEbpfManager::new("/nonexistent/path".to_string());
        // Should report not available since the .o file doesn't exist
        assert!(!crate::ebpf::EbpfManager::is_available(&mgr));
    }
}
