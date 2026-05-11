//! Version compatibility matrix enforcement.
//!
//! Checks that components in the cluster are running compatible versions.
//! The compatibility matrix can be loaded from a TOML configuration file.

use chv_errors::ChvError;
use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

/// Components in the CHV cluster.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Component {
    Agent,
    Stord,
    Nwd,
    ControlPlane,
    Chvctl,
}

impl Component {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Stord => "stord",
            Self::Nwd => "nwd",
            Self::ControlPlane => "controlplane",
            Self::Chvctl => "chvctl",
        }
    }
}

impl std::fmt::Display for Component {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A single compatibility entry specifying the allowed version range for a component.
#[derive(Debug, Clone, Deserialize)]
pub struct CompatEntry {
    pub component: Component,
    #[serde(deserialize_with = "deserialize_version")]
    pub min_version: Version,
    #[serde(deserialize_with = "deserialize_version")]
    pub max_version: Version,
}

/// Report of a version incompatibility.
#[derive(Debug, Clone)]
pub struct IncompatibilityReport {
    pub component: Component,
    pub current_version: String,
    pub min_allowed: String,
    pub max_allowed: String,
    pub message: String,
}

/// Compatibility matrix defining allowed version ranges for all components.
#[derive(Debug, Clone, Deserialize)]
pub struct CompatibilityMatrix {
    #[serde(rename = "entry")]
    entries: Vec<CompatEntry>,
}

/// TOML wrapper for deserialization.
#[derive(Debug, Deserialize)]
struct CompatibilityMatrixFile {
    #[serde(rename = "compatibility")]
    matrix: CompatibilityMatrixInner,
}

#[derive(Debug, Deserialize)]
struct CompatibilityMatrixInner {
    #[serde(rename = "entry")]
    entries: Vec<CompatEntry>,
}

fn deserialize_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Version::parse(&s).map_err(serde::de::Error::custom)
}

impl CompatibilityMatrix {
    /// Create a new compatibility matrix from entries.
    pub fn new(entries: Vec<CompatEntry>) -> Self {
        Self { entries }
    }

    /// Load compatibility matrix from a TOML file.
    ///
    /// Expected format:
    /// ```toml
    /// [compatibility]
    /// [[compatibility.entry]]
    /// component = "agent"
    /// min_version = "0.1.0"
    /// max_version = "1.0.0"
    ///
    /// [[compatibility.entry]]
    /// component = "stord"
    /// min_version = "0.1.0"
    /// max_version = "1.0.0"
    /// ```
    pub fn load_from_file(path: &Path) -> Result<Self, ChvError> {
        let content = std::fs::read_to_string(path).map_err(|e| ChvError::Io {
            path: path.display().to_string(),
            source: e,
        })?;

        let file: CompatibilityMatrixFile =
            toml::from_str(&content).map_err(|e| ChvError::Internal {
                reason: format!("failed to parse compatibility matrix: {}", e),
            })?;

        info!(
            path = %path.display(),
            entries = file.matrix.entries.len(),
            "loaded compatibility matrix"
        );

        Ok(Self {
            entries: file.matrix.entries,
        })
    }

    /// Check if a specific component version is compatible with the matrix.
    pub fn is_compatible(&self, component: Component, version: &str) -> Result<bool, ChvError> {
        let parsed = Version::parse(version).map_err(|e| ChvError::InvalidArgument {
            field: "version".to_string(),
            reason: format!("invalid semver '{}': {}", version, e),
        })?;

        for entry in &self.entries {
            if entry.component == component {
                let compatible = parsed >= entry.min_version && parsed <= entry.max_version;
                debug!(
                    component = %component,
                    version = %version,
                    min = %entry.min_version,
                    max = %entry.max_version,
                    compatible = compatible,
                    "version compatibility check"
                );
                return Ok(compatible);
            }
        }

        // No entry for this component — assume compatible (no restrictions)
        debug!(
            component = %component,
            version = %version,
            "no compatibility entry found, assuming compatible"
        );
        Ok(true)
    }

    /// Check all component versions against the matrix.
    ///
    /// Returns a list of incompatibility reports for any versions that fall
    /// outside the allowed range.
    pub fn check_all(&self, versions: &HashMap<Component, String>) -> Vec<IncompatibilityReport> {
        let mut reports = Vec::new();

        for (component, version) in versions {
            let parsed = match Version::parse(version) {
                Ok(v) => v,
                Err(e) => {
                    reports.push(IncompatibilityReport {
                        component: *component,
                        current_version: version.clone(),
                        min_allowed: "N/A".to_string(),
                        max_allowed: "N/A".to_string(),
                        message: format!("invalid version '{}': {}", version, e),
                    });
                    continue;
                }
            };

            for entry in &self.entries {
                if entry.component == *component
                    && (parsed < entry.min_version || parsed > entry.max_version)
                {
                    let report = IncompatibilityReport {
                        component: *component,
                        current_version: version.clone(),
                        min_allowed: entry.min_version.to_string(),
                        max_allowed: entry.max_version.to_string(),
                        message: format!(
                            "{} version {} is outside allowed range [{}, {}]",
                            component, version, entry.min_version, entry.max_version
                        ),
                    };
                    warn!(
                        component = %component,
                        version = %version,
                        min = %entry.min_version,
                        max = %entry.max_version,
                        "version incompatibility detected"
                    );
                    reports.push(report);
                }
            }
        }

        reports
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_matrix() -> CompatibilityMatrix {
        CompatibilityMatrix::new(vec![
            CompatEntry {
                component: Component::Agent,
                min_version: Version::new(0, 1, 0),
                max_version: Version::new(1, 0, 0),
            },
            CompatEntry {
                component: Component::Stord,
                min_version: Version::new(0, 2, 0),
                max_version: Version::new(1, 0, 0),
            },
            CompatEntry {
                component: Component::ControlPlane,
                min_version: Version::new(0, 1, 0),
                max_version: Version::new(2, 0, 0),
            },
        ])
    }

    #[test]
    fn test_compatible_version() {
        let matrix = test_matrix();
        assert!(matrix.is_compatible(Component::Agent, "0.5.0").unwrap());
        assert!(matrix.is_compatible(Component::Agent, "0.1.0").unwrap());
        assert!(matrix.is_compatible(Component::Agent, "1.0.0").unwrap());
    }

    #[test]
    fn test_incompatible_version_too_low() {
        let matrix = test_matrix();
        assert!(!matrix.is_compatible(Component::Agent, "0.0.9").unwrap());
    }

    #[test]
    fn test_incompatible_version_too_high() {
        let matrix = test_matrix();
        assert!(!matrix.is_compatible(Component::Agent, "1.0.1").unwrap());
    }

    #[test]
    fn test_unknown_component_is_compatible() {
        let matrix = test_matrix();
        // Chvctl has no entry in the test matrix
        assert!(matrix.is_compatible(Component::Chvctl, "5.0.0").unwrap());
    }

    #[test]
    fn test_invalid_version_returns_error() {
        let matrix = test_matrix();
        let result = matrix.is_compatible(Component::Agent, "not-a-version");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_all_no_issues() {
        let matrix = test_matrix();
        let mut versions = HashMap::new();
        versions.insert(Component::Agent, "0.5.0".to_string());
        versions.insert(Component::Stord, "0.3.0".to_string());
        versions.insert(Component::ControlPlane, "1.0.0".to_string());

        let reports = matrix.check_all(&versions);
        assert!(reports.is_empty());
    }

    #[test]
    fn test_check_all_with_issues() {
        let matrix = test_matrix();
        let mut versions = HashMap::new();
        versions.insert(Component::Agent, "2.0.0".to_string()); // too high
        versions.insert(Component::Stord, "0.1.0".to_string()); // too low

        let reports = matrix.check_all(&versions);
        assert_eq!(reports.len(), 2);
    }

    #[test]
    fn test_check_all_invalid_version() {
        let matrix = test_matrix();
        let mut versions = HashMap::new();
        versions.insert(Component::Agent, "invalid".to_string());

        let reports = matrix.check_all(&versions);
        assert_eq!(reports.len(), 1);
        assert!(reports[0].message.contains("invalid version"));
    }

    #[test]
    fn test_component_display() {
        assert_eq!(Component::Agent.as_str(), "agent");
        assert_eq!(Component::ControlPlane.as_str(), "controlplane");
        assert_eq!(Component::Chvctl.as_str(), "chvctl");
    }

    #[test]
    fn test_load_from_toml_string() {
        let toml_content = r#"
[compatibility]
[[compatibility.entry]]
component = "agent"
min_version = "0.1.0"
max_version = "1.0.0"

[[compatibility.entry]]
component = "stord"
min_version = "0.2.0"
max_version = "1.0.0"
"#;
        let file: CompatibilityMatrixFile = toml::from_str(toml_content).unwrap();
        let matrix = CompatibilityMatrix::new(file.matrix.entries);
        assert_eq!(matrix.entries.len(), 2);
        assert!(matrix.is_compatible(Component::Agent, "0.5.0").unwrap());
    }
}
