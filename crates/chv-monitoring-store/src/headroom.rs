use std::path::Path;

/// Available space on the filesystem holding `path`, in bytes.
///
/// ADR-027: a separate file on the same filesystem does NOT isolate
/// disk-full risk — the monitoring store checks real headroom before
/// accepting batches and degrades (stops ingesting) when the floor is
/// hit, so monitoring can never fill the disk that VM lifecycle
/// depends on.
pub fn available_bytes(path: &Path) -> std::io::Result<u64> {
    let stats = nix::sys::statvfs::statvfs(path)
        .map_err(|e| std::io::Error::other(format!("statvfs({}): {e}", path.display())))?;
    let available = stats.blocks_available() as u64 * stats.fragment_size() as u64;
    Ok(available)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_real_headroom() {
        // Hermetic within reason: /tmp is a real filesystem the test
        // runner can always stat. The value is not asserted beyond
        // sanity (an empty disk is a legitimate reading).
        let avail = available_bytes(Path::new("/tmp")).unwrap();
        assert!(avail < u64::MAX / 2, "implausible headroom: {avail}");
    }
}
