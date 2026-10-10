//! `/proc/self/mounts` + `statvfs` filesystem family (G4).
//!
//! Discovery is a pure parse of the mounts file (kernel-provided,
//! one mount per line: device, mountpoint, fstype, options...).
//! Sizes and inode counts come from `statvfs` per mountpoint — the
//! family's only impure step, kept in a thin function so tests
//! inject `StatData` instead of touching the filesystem.

use crate::CollectedSample;

pub(crate) const FS_AVAILABLE_BYTES: &str = "vm.guest.fs.available_bytes";
pub(crate) const FS_TOTAL_BYTES: &str = "vm.guest.fs.total_bytes";
pub(crate) const FS_INODES_UTILIZATION: &str = "vm.guest.fs.inodes_utilization_ratio";
pub(crate) const FS_READ_ONLY: &str = "vm.guest.fs.read_only";

/// Discovery bound: a pathological mounts file (a container with
/// hundreds of bind mounts) must not blow up the batch. The 65th
/// mount is skipped.
const MAX_MOUNTS: usize = 64;
/// Dimension bound for `mount_id`. A mount whose id would exceed it
/// is dropped whole — honest absence, never a mid-UTF-8 truncation.
const MAX_MOUNT_ID_BYTES: usize = 128;

/// fstypes that carry real data. The allowlist subsumes the pseudo
/// filesystems (proc, sysfs, devpts, devtmpfs, cgroup*, hugetlbfs,
/// mqueue, fusectl, securityfs, debugfs, tracefs, configfs, pstore,
/// autofs, binfmt_misc, rpc_pipefs): anything not listed is skipped.
const REAL_FSTYPES: &[&str] = &[
    "ext2", "ext3", "ext4", "xfs", "btrfs", "zfs", "tmpfs", "vfat", "exfat", "nfs", "nfs4", "f2fs",
    "overlay", "squashfs",
];

/// One parsed mounts line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MountEntry {
    pub device: String,
    /// Octal-escaped mountpoint, decoded (the mounts file writes
    /// spaces as `\040` etc.).
    pub mountpoint: String,
    pub fstype: String,
    /// From the mount options field (not a statvfs flag).
    pub read_only: bool,
}

/// Pure parse of `/proc/self/mounts` contents: real filesystems
/// only, bounded to 64 mounts. Malformed lines are skipped.
pub(crate) fn parse_mounts(mounts: &str) -> Vec<MountEntry> {
    let mut out = Vec::new();
    for line in mounts.lines() {
        if out.len() >= MAX_MOUNTS {
            break;
        }
        let mut fields = line.split_whitespace();
        let Some(device) = fields.next() else {
            continue;
        };
        let Some(mountpoint) = fields.next() else {
            continue;
        };
        let Some(fstype) = fields.next() else {
            continue;
        };
        let Some(options) = fields.next() else {
            continue;
        };
        if !REAL_FSTYPES.contains(&fstype) {
            continue;
        }
        out.push(MountEntry {
            device: device.to_string(),
            mountpoint: decode_octal(mountpoint),
            fstype: fstype.to_string(),
            // The options field is a comma list; `ro` must match a
            // whole option, not a substring of something else.
            read_only: options.split(',').any(|o| o == "ro"),
        });
    }
    out
}

/// Decode the mounts-file escaping: the kernel writes spaces, tabs,
/// newlines and backslashes inside mountpoints as three octal
/// digits after a backslash (`\040`, `\011`, `\012`, `\134`). Any
/// other backslash sequence stays a literal backslash. Byte-level
/// arithmetic only — the field is valid UTF-8 but a malformed
/// multi-byte sequence after a backslash must never panic on a
/// string-slice boundary, and `\777`-style values above 255 stay
/// literal rather than wrap.
fn decode_octal(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() {
            let digits = [b[i + 1], b[i + 2], b[i + 3]];
            if digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                let v = (digits[0] - b'0') as u32 * 64
                    + (digits[1] - b'0') as u32 * 8
                    + (digits[2] - b'0') as u32;
                if v <= 255 {
                    out.push(v as u8);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Plain statvfs result, decoupled from libc so tests inject values
/// instead of touching the filesystem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StatData {
    pub f_bavail: u64,
    pub f_blocks: u64,
    pub f_frsize: u64,
    pub f_files: u64,
    pub f_ffree: u64,
}

/// statvfs(3) on one mountpoint. `std` has no statvfs API — the
/// single reason this crate depends on `libc`. Failure (including a
/// mountpoint with an interior NUL, which cannot become a C string)
/// is an honest absence.
pub(crate) fn statvfs_mount(mountpoint: &str) -> Option<StatData> {
    let path = std::ffi::CString::new(mountpoint).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is a valid NUL-terminated string and `st` is an
    // out-parameter of the matching libc type. Read-only call.
    if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some(StatData {
        f_bavail: st.f_bavail as u64,
        f_blocks: st.f_blocks as u64,
        f_frsize: st.f_frsize as u64,
        f_files: st.f_files as u64,
        f_ffree: st.f_ffree as u64,
    })
}

/// Map one mount + one statvfs result to samples. `stat == None`
/// (statvfs failure) skips the size/inode fields but keeps the
/// options-derived `read_only`, which never depended on statvfs. A
/// mount whose `mount_id` would exceed 128 bytes is dropped whole
/// (honest absence, never a mid-UTF-8 truncation). A zero
/// denominator (f_files == 0) leaves the inode ratio absent; a
/// byte-count that would overflow is skipped, never wrapped.
pub(crate) fn emit_mount(entry: &MountEntry, stat: Option<StatData>) -> Vec<CollectedSample> {
    let mount_id = format!("{}:{}", entry.fstype, entry.mountpoint);
    if mount_id.len() > MAX_MOUNT_ID_BYTES {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(4);
    out.push(CollectedSample::integer_with_dimension(
        FS_READ_ONLY,
        u64::from(entry.read_only),
        "mount_id",
        mount_id.clone(),
    ));
    let Some(stat) = stat else {
        return out;
    };
    if let Some(available) = stat.f_bavail.checked_mul(stat.f_frsize) {
        out.push(CollectedSample::integer_with_dimension(
            FS_AVAILABLE_BYTES,
            available,
            "mount_id",
            mount_id.clone(),
        ));
    }
    if let Some(total) = stat.f_blocks.checked_mul(stat.f_frsize) {
        out.push(CollectedSample::integer_with_dimension(
            FS_TOTAL_BYTES,
            total,
            "mount_id",
            mount_id.clone(),
        ));
    }
    if stat.f_files > 0 {
        // f_ffree > f_files is a kernel accounting anomaly; clamp
        // rather than emit a negative ratio.
        let used = stat.f_files.saturating_sub(stat.f_ffree);
        let ratio = (used as f64 / stat.f_files as f64).clamp(0.0, 1.0);
        out.push(CollectedSample::float_with_dimension(
            FS_INODES_UTILIZATION,
            ratio,
            "mount_id",
            mount_id,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0\n\
                          proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
                          udev /dev devtmpfs rw,nosuid,relatime,size=8171232k 0 0\n\
                          cgroup2 /sys/fs/cgroup cgroup2 rw,nosuid,nodev,noexec 0 0\n\
                          /dev/vda1 / ext4 rw,relatime,errors=remount-ro 0 0\n\
                          /dev/vdb1 /data xfs rw,relatime,attr2 0 0\n\
                          tmpfs /run tmpfs rw,nosuid,nodev,noexec,relatime 0 0\n\
                          nfs4 /mnt/share nfs4 ro,relatime,vers=4.2 0 0\n\
                          /dev/mapper/crypt /home\\040user ext4 rw,relatime 0 0\n";

    fn ext4_entry(mountpoint: &str, read_only: bool) -> MountEntry {
        MountEntry {
            device: "/dev/vda1".into(),
            mountpoint: mountpoint.into(),
            fstype: "ext4".into(),
            read_only,
        }
    }

    #[test]
    fn parses_real_mounts_and_decodes_octal() {
        let mounts = parse_mounts(MOUNTS);
        assert_eq!(mounts.len(), 5, "pseudo filesystems are skipped");
        assert_eq!(mounts[0].mountpoint, "/");
        assert_eq!(mounts[0].fstype, "ext4");
        assert!(!mounts[0].read_only);
        assert_eq!(mounts[3].fstype, "nfs4");
        assert!(mounts[3].read_only, "options-field ro flag");
        assert_eq!(
            mounts[4].mountpoint, "/home user",
            "\\040 must decode to a space"
        );
    }

    #[test]
    fn garbage_is_absence() {
        assert!(parse_mounts("").is_empty());
        assert!(parse_mounts("not enough fields\nalso not\n").is_empty());
        assert!(parse_mounts("/dev/vda1\n").is_empty());
    }

    #[test]
    fn bounds_at_64_mounts() {
        let text: String = (0..65)
            .map(|i| format!("/dev/vd{i} /mnt{i} ext4 rw 0 0\n"))
            .collect();
        assert_eq!(parse_mounts(&text).len(), 64, "65th mount is skipped");
        // Pseudo mounts do not consume the bound.
        let mixed = "proc /proc proc rw 0 0\n".to_string() + &text;
        assert_eq!(parse_mounts(&mixed).len(), 64);
    }

    #[test]
    fn read_only_comes_from_options_not_statvfs() {
        // statvfs failure must not affect the options-derived flag.
        let out = emit_mount(&ext4_entry("/", true), None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].metric_id, FS_READ_ONLY);
        assert_eq!(out[0].value, crate::SampleValue::Integer(1));
    }

    #[test]
    fn statvfs_emission_mapping() {
        let stat = StatData {
            f_bavail: 100,
            f_blocks: 200,
            f_frsize: 4096,
            f_files: 1000,
            f_ffree: 250,
        };
        let out = emit_mount(&ext4_entry("/", false), Some(stat));
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].value, crate::SampleValue::Integer(0), "rw mount");
        assert_eq!(
            out[1],
            CollectedSample::integer_with_dimension(
                FS_AVAILABLE_BYTES,
                100 * 4096,
                "mount_id",
                "ext4:/".into()
            )
        );
        assert_eq!(
            out[2].value,
            crate::SampleValue::Integer(200 * 4096),
            "total bytes"
        );
        match out[3].value {
            crate::SampleValue::Float(v) => assert!((v - 0.75).abs() < 1e-9),
            other => panic!("inode ratio must be a float, got {other:?}"),
        }
        let (k, v) = out[3].dimension.as_ref().unwrap();
        assert_eq!((*k, v.as_str()), ("mount_id", "ext4:/"));
    }

    #[test]
    fn zero_inode_denominator_is_absence() {
        let stat = StatData {
            f_bavail: 1,
            f_blocks: 1,
            f_frsize: 4096,
            f_files: 0,
            f_ffree: 0,
        };
        let out = emit_mount(&ext4_entry("/", false), Some(stat));
        assert_eq!(out.len(), 3, "inode ratio is absent when f_files == 0");
        assert!(!out.iter().any(|s| s.metric_id == FS_INODES_UTILIZATION));
    }

    #[test]
    fn inode_anomaly_clamps_to_zero() {
        let stat = StatData {
            f_bavail: 1,
            f_blocks: 1,
            f_frsize: 4096,
            f_files: 100,
            f_ffree: 200, // more free than total: impossible
        };
        let out = emit_mount(&ext4_entry("/", false), Some(stat));
        match out
            .iter()
            .find(|s| s.metric_id == FS_INODES_UTILIZATION)
            .unwrap()
            .value
        {
            crate::SampleValue::Float(v) => assert_eq!(v, 0.0),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn byte_overflow_is_absence() {
        let stat = StatData {
            f_bavail: u64::MAX,
            f_blocks: u64::MAX,
            f_frsize: 2,
            f_files: 10,
            f_ffree: 5,
        };
        let out = emit_mount(&ext4_entry("/", false), Some(stat));
        assert!(!out.iter().any(|s| s.metric_id == FS_AVAILABLE_BYTES));
        assert!(!out.iter().any(|s| s.metric_id == FS_TOTAL_BYTES));
    }

    #[test]
    fn over_long_mount_id_drops_the_mount() {
        let entry = ext4_entry(&"/very/long".repeat(20), false);
        assert!(emit_mount(&entry, None).is_empty());
        // 128 bytes exactly is fine; 129 is not.
        let ok = format!("ext4:{}", "x".repeat(128 - "ext4:".len()));
        assert_eq!(ok.len(), 128);
        let entry = MountEntry {
            mountpoint: ok["ext4:".len()..].into(),
            ..ext4_entry("", false)
        };
        assert_eq!(emit_mount(&entry, None).len(), 1);
    }

    #[test]
    fn decode_octal_never_panics_on_multibyte_garbage() {
        // A backslash followed by multi-byte UTF-8 used to hit a
        // string-slice boundary panic; garbage must stay absence,
        // never panic the agent.
        assert_eq!(decode_octal("\\ü040"), "\\ü040");
        assert_eq!(decode_octal("\\ü"), "\\ü");
        // Values above 255 stay literal instead of wrapping.
        assert_eq!(decode_octal("\\777"), "\\777");
        // A truncated escape at end-of-field stays literal.
        assert_eq!(decode_octal("\\04"), "\\04");
        assert_eq!(decode_octal("\\040"), " ");
    }
}
