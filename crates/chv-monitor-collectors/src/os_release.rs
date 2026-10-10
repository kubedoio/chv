//! `/etc/os-release` + `/proc/sys/kernel/osrelease` identity for the
//! batch envelope. Read-only, allowlisted fields only.

/// OS identity metadata (envelope `os` object). All fields optional;
/// unparseable sources simply leave them out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OsIdentity {
    pub name: Option<String>,
    pub version: Option<String>,
    pub kernel_release: Option<String>,
}

/// Parse `NAME`/`VERSION_ID` (falling back to `VERSION`) from
/// os-release contents and take the kernel release verbatim from
/// `/proc/sys/kernel/osrelease`.
pub(crate) fn parse_identity(os_release: &str, kernel_release: &str) -> OsIdentity {
    let mut name = None;
    let mut version_id = None;
    let mut version = None;
    for line in os_release.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        // Strip shell-style quoting from the value.
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key.trim() {
            "NAME" => name = Some(value.to_string()),
            "VERSION_ID" => version_id = Some(value.to_string()),
            "VERSION" => version = Some(value.to_string()),
            _ => {}
        }
    }
    let kernel = kernel_release.trim();
    OsIdentity {
        // The manager bounds each field to 64 bytes; truncate rather
        // than have the whole batch rejected over a verbose NAME.
        name: name.map(truncate64),
        // VERSION_ID is the machine-readable field; VERSION (a prose
        // string like "24.04.1 LTS (Noble Numbat)") is the fallback.
        version: version_id.or(version).map(truncate64),
        kernel_release: (!kernel.is_empty()).then(|| truncate64(kernel.to_string())),
    }
}

fn truncate64(v: String) -> String {
    match v.char_indices().nth(64) {
        Some((idx, _)) => v[..idx].to_string(),
        None => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UBUNTU: &str = "NAME=\"Ubuntu\"\nVERSION=\"24.04.1 LTS (Noble Numbat)\"\nID=ubuntu\nVERSION_ID=\"24.04\"\n";

    #[test]
    fn parses_os_release_preferring_version_id() {
        let id = parse_identity(UBUNTU, "6.8.0-42-generic\n");
        assert_eq!(id.name.as_deref(), Some("Ubuntu"));
        assert_eq!(id.version.as_deref(), Some("24.04"));
        assert_eq!(id.kernel_release.as_deref(), Some("6.8.0-42-generic"));
    }

    #[test]
    fn falls_back_to_version_without_id() {
        let id = parse_identity("NAME='Debian'\nVERSION=\"12 (bookworm)\"\n", "");
        assert_eq!(id.name.as_deref(), Some("Debian"));
        assert_eq!(id.version.as_deref(), Some("12 (bookworm)"));
        assert_eq!(id.kernel_release, None);
    }

    #[test]
    fn empty_sources_are_absent() {
        assert_eq!(parse_identity("", ""), OsIdentity::default());
    }

    #[test]
    fn long_values_are_truncated_to_64() {
        let id = parse_identity(
            "NAME=0123456789012345678901234567890123456789012345678901234567890123456789\n",
            "",
        );
        assert_eq!(id.name.unwrap().len(), 64);
    }
}
