//! Crash-durable file writes for identity-bearing agent state.
//!
//! `vm.reboot` (and any hard VM reset) tears the machine down without
//! an ACPI shutdown — the guest kernel gets no chance to flush its
//! page cache. A file that was written (or replaced via rename)
//! seconds before the reset can therefore come back EMPTY: the rename
//! was durable while the data blocks it points at were never written.
//! This is not hypothetical — the G5 rig observed exactly that (a
//! config rewritten shortly before `vm.reboot` came back zero-length
//! after the reset).
//!
//! The agent's identity files (credential, state, spool entries) must
//! survive that, so every write goes: tmp file → fsync data → rename
//! → fsync parent directory. The tmp+rename also guarantees that a
//! crash mid-write can never leave a torn file at the final path.

use std::io;
use std::path::Path;

/// Atomically replace `path` with `bytes`, durably. The data reaches
/// the disk BEFORE the rename does, and the rename itself is made
/// durable by syncing the parent directory afterwards. On unix the
/// new file is created with `mode`; the not-unix fallback keeps
/// non-linux builds compiling (this crate ships on linux).
pub(crate) fn write_durable(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    write_file_synced(&tmp, bytes, mode)?;
    std::fs::rename(&tmp, path)?;
    if let Some(dir) = path.parent() {
        // Best-effort: a failure to sync the directory degrades to
        // "durable after the next flush", never to torn content.
        let _ = std::fs::File::open(dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

#[cfg(unix)]
fn write_file_synced(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_file_synced(path: &Path, bytes: &[u8], _mode: u32) -> io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_durable_replaces_content_and_leaves_no_tmp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        write_durable(&path, b"first", 0o600).unwrap();
        write_durable(&path, b"second", 0o600).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn write_durable_creates_parent_dirs_caller_must_not() {
        // The caller guarantees the parent exists (state_dir is
        // created at startup); this pins the error shape if it does
        // not, so a regression surfaces as a loud startup failure.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("state.json");
        assert!(write_durable(&path, b"x", 0o600).is_err());
    }
}
