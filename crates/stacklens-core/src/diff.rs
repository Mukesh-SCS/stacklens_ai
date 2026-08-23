use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Result;
use crate::snapshot::EnvironmentSnapshot;

/// Metadata fields that always differ between separately captured snapshots
/// and are reported on [`SnapshotDiff`] itself.
const IGNORED_PATHS: &[&str] = &["captured_at"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SnapshotDiff {
    pub schema_version: u32,
    pub left_captured_at: String,
    pub right_captured_at: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldChange {
    pub path: String,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeKind {
    Added { right: Value },
    Removed { left: Value },
    Changed { left: Value, right: Value },
}

pub fn diff_snapshots(
    left: &EnvironmentSnapshot,
    right: &EnvironmentSnapshot,
) -> Result<SnapshotDiff> {
    let left_value = serde_json::to_value(left)?;
    let right_value = serde_json::to_value(right)?;
    let mut changes = Vec::new();
    walk(&left_value, &right_value, "", &mut changes);
    changes.retain(|change| !IGNORED_PATHS.contains(&change.path.as_str()));

    Ok(SnapshotDiff {
        schema_version: left.schema_version.max(right.schema_version),
        left_captured_at: left.captured_at.clone(),
        right_captured_at: right.captured_at.clone(),
        changes,
    })
}

fn walk(left: &Value, right: &Value, path: &str, out: &mut Vec<FieldChange>) {
    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let keys: BTreeSet<&String> = left_map.keys().chain(right_map.keys()).collect();
            for key in keys {
                let child = child_path(path, key);
                match (left_map.get(key), right_map.get(key)) {
                    (Some(left_child), Some(right_child)) => {
                        walk(left_child, right_child, &child, out);
                    }
                    (Some(left_child), None) => out.push(FieldChange {
                        path: child,
                        kind: ChangeKind::Removed {
                            left: left_child.clone(),
                        },
                    }),
                    (None, Some(right_child)) => out.push(FieldChange {
                        path: child,
                        kind: ChangeKind::Added {
                            right: right_child.clone(),
                        },
                    }),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(left_items), Value::Array(right_items)) => {
            let len = left_items.len().max(right_items.len());
            for index in 0..len {
                let child = format!("{path}[{index}]");
                match (left_items.get(index), right_items.get(index)) {
                    (Some(left_child), Some(right_child)) => {
                        walk(left_child, right_child, &child, out);
                    }
                    (Some(left_child), None) => out.push(FieldChange {
                        path: child,
                        kind: ChangeKind::Removed {
                            left: left_child.clone(),
                        },
                    }),
                    (None, Some(right_child)) => out.push(FieldChange {
                        path: child,
                        kind: ChangeKind::Added {
                            right: right_child.clone(),
                        },
                    }),
                    (None, None) => {}
                }
            }
        }
        (left, right) if left == right => {}
        (left, right) => out.push(FieldChange {
            path: path.to_owned(),
            kind: ChangeKind::Changed {
                left: left.clone(),
                right: right.clone(),
            },
        }),
    }
}

fn child_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_owned()
    } else {
        format!("{parent}.{key}")
    }
}

#[cfg(test)]
mod tests {
    use super::{diff_snapshots, ChangeKind};
    use crate::model::{ContainerDetection, Endian, EnvironmentInfo, SystemInfo, WslDetection};
    use crate::probe::Probe;
    use crate::snapshot::{EnvironmentSnapshot, SNAPSHOT_SCHEMA_VERSION};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn base_system(os: &str) -> SystemInfo {
        SystemInfo {
            os: os.to_owned(),
            arch: "x86_64".to_owned(),
            family: "unix".to_owned(),
            pointer_width: 64,
            endian: Endian::Little,
            os_id: Some("ubuntu".to_owned()),
            os_id_like: None,
            os_pretty_name: None,
            os_version_id: None,
            kernel_release: Some("6.8.0".to_owned()),
            container: ContainerDetection::NotDetected,
            wsl: WslDetection::NotDetected,
        }
    }

    fn snapshot(captured_at: &str, os: &str, ld_library_path: Option<&str>) -> EnvironmentSnapshot {
        let mut variables = BTreeMap::new();
        if let Some(value) = ld_library_path {
            variables.insert("LD_LIBRARY_PATH".to_owned(), value.to_owned());
        }
        EnvironmentSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            stacklens_version: "0.1.0".to_owned(),
            captured_at: captured_at.to_owned(),
            system: Probe::collected(base_system(os)),
            environment: Probe::collected(EnvironmentInfo { variables }),
            gpu: Probe::NotImplemented,
            nvidia_driver: Probe::NotImplemented,
            cuda: Probe::NotImplemented,
            libraries: Probe::NotImplemented,
            torch: Probe::NotImplemented,
        }
    }

    #[test]
    fn identical_payloads_have_no_changes() {
        let left = snapshot("2026-01-01T00:00:00Z", "linux", Some("/usr/lib"));
        let right = snapshot("2026-01-02T00:00:00Z", "linux", Some("/usr/lib"));
        let diff = diff_snapshots(&left, &right).unwrap();
        assert!(diff.changes.is_empty());
        assert_eq!(diff.left_captured_at, "2026-01-01T00:00:00Z");
        assert_eq!(diff.right_captured_at, "2026-01-02T00:00:00Z");
    }

    #[test]
    fn reports_changed_os() {
        let left = snapshot("2026-01-01T00:00:00Z", "linux", None);
        let right = snapshot("2026-01-01T00:00:00Z", "windows", None);
        let diff = diff_snapshots(&left, &right).unwrap();
        let change = diff
            .changes
            .iter()
            .find(|change| change.path == "system.data.os")
            .expect("os change");
        match &change.kind {
            ChangeKind::Changed { left, right } => {
                assert_eq!(left, &json!("linux"));
                assert_eq!(right, &json!("windows"));
            }
            other => panic!("unexpected change: {other:?}"),
        }
    }

    #[test]
    fn reports_added_and_removed_env() {
        let left = snapshot("2026-01-01T00:00:00Z", "linux", Some("/opt/cuda/lib64"));
        let right = snapshot("2026-01-01T00:00:00Z", "linux", None);
        let diff = diff_snapshots(&left, &right).unwrap();
        assert!(diff.changes.iter().any(|change| {
            change.path == "environment.data.variables.LD_LIBRARY_PATH"
                && matches!(change.kind, ChangeKind::Removed { .. })
        }));
    }

    #[test]
    fn loads_workspace_fixtures() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let left_path = root.join("fixtures/snapshot/left.json");
        let right_path = root.join("fixtures/snapshot/right.json");
        let left = EnvironmentSnapshot::from_json_str(&std::fs::read_to_string(left_path).unwrap())
            .unwrap();
        let right =
            EnvironmentSnapshot::from_json_str(&std::fs::read_to_string(right_path).unwrap())
                .unwrap();
        let diff = diff_snapshots(&left, &right).unwrap();
        assert!(diff
            .changes
            .iter()
            .any(|change| change.path == "system.data.os"));
        assert!(diff
            .changes
            .iter()
            .any(|change| change.path.contains("LD_LIBRARY_PATH")));
    }
}
