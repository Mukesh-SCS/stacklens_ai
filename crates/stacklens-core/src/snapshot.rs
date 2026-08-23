use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::{
    CudaInfo, EnvironmentInfo, GpuInfo, LibraryInfo, NvidiaDriverInfo, SystemInfo, TorchInfo,
};
use crate::probe::Probe;

pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Versioned machine/environment snapshot.
///
/// Collectors that are not written yet serialize as `not_implemented`.
/// Do not populate GPU, CUDA, library, or PyTorch fields with sample data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EnvironmentSnapshot {
    pub schema_version: u32,
    pub stacklens_version: String,
    pub captured_at: String,
    #[serde(default)]
    pub system: Probe<SystemInfo>,
    #[serde(default)]
    pub environment: Probe<EnvironmentInfo>,
    #[serde(default)]
    pub gpu: Probe<GpuInfo>,
    #[serde(default)]
    pub nvidia_driver: Probe<NvidiaDriverInfo>,
    #[serde(default)]
    pub cuda: Probe<CudaInfo>,
    #[serde(default)]
    pub libraries: Probe<Vec<LibraryInfo>>,
    #[serde(default)]
    pub torch: Probe<TorchInfo>,
}

impl EnvironmentSnapshot {
    pub fn from_json_str(text: &str) -> Result<Self> {
        let snapshot: Self = serde_json::from_str(text)?;
        snapshot.check_schema()?;
        Ok(snapshot)
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    fn check_schema(&self) -> Result<()> {
        if self.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema {
                found: self.schema_version,
                supported: SNAPSHOT_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{EnvironmentSnapshot, SNAPSHOT_SCHEMA_VERSION};
    use crate::error::Error;
    use crate::probe::Probe;

    fn empty_snapshot() -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            stacklens_version: "0.1.0".to_owned(),
            captured_at: "2026-01-01T00:00:00Z".to_owned(),
            system: Probe::NotImplemented,
            environment: Probe::NotImplemented,
            gpu: Probe::NotImplemented,
            nvidia_driver: Probe::NotImplemented,
            cuda: Probe::NotImplemented,
            libraries: Probe::NotImplemented,
            torch: Probe::NotImplemented,
        }
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SNAPSHOT_SCHEMA_VERSION, 1);
    }

    #[test]
    fn json_roundtrip() {
        let snapshot = empty_snapshot();
        let json = snapshot.to_json_string().unwrap();
        let parsed = EnvironmentSnapshot::from_json_str(&json).unwrap();
        assert_eq!(parsed, snapshot);
        assert!(json.contains("\"schema_version\": 1"));
        assert!(json.contains("\"status\": \"not_implemented\""));
    }

    #[test]
    fn rejects_unknown_schema() {
        let mut snapshot = empty_snapshot();
        snapshot.schema_version = 99;
        let json = snapshot.to_json_string().unwrap();
        let err = EnvironmentSnapshot::from_json_str(&json).unwrap_err();
        assert!(matches!(
            err,
            Error::UnsupportedSchema {
                found: 99,
                supported: 1
            }
        ));
    }

    #[test]
    fn missing_optional_probes_default_to_not_implemented() {
        let json = r#"{
            "schema_version": 1,
            "stacklens_version": "0.1.0",
            "captured_at": "2026-01-01T00:00:00Z"
        }"#;
        let parsed = EnvironmentSnapshot::from_json_str(json).unwrap();
        assert!(matches!(parsed.gpu, Probe::NotImplemented));
        assert!(matches!(parsed.torch, Probe::NotImplemented));
    }

    #[test]
    fn phase1b_snapshot_with_unimplemented_cuda_still_loads() {
        let json = include_str!("../../../fixtures/snapshot/phase1b.json");
        let parsed = EnvironmentSnapshot::from_json_str(json).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert!(matches!(parsed.cuda, Probe::NotImplemented));
        assert!(matches!(parsed.libraries, Probe::NotImplemented));
        assert!(matches!(parsed.torch, Probe::NotImplemented));
        assert!(parsed.system.as_collected().is_some());
    }

    #[test]
    fn phase1a_placeholder_cuda_object_defaults() {
        let json = r#"{
            "schema_version": 1,
            "stacklens_version": "0.1.0",
            "captured_at": "2026-01-01T00:00:00Z",
            "cuda": {
                "status": "collected",
                "data": {
                    "driver_cuda_version": "12.8",
                    "toolkit_version": "12.4",
                    "nvcc_version": "12.4.131",
                    "toolkit_paths": ["/usr/local/cuda"],
                    "runtime_libraries": []
                }
            }
        }"#;
        let parsed = EnvironmentSnapshot::from_json_str(json).unwrap();
        let cuda = parsed.cuda.as_collected().expect("cuda");
        assert!(cuda.toolkits.is_empty());
        assert!(cuda.driver_api_capability.is_none());
    }

    #[test]
    fn phase1a_collected_probe_without_issues_still_loads() {
        let json = r#"{
            "schema_version": 1,
            "stacklens_version": "0.1.0",
            "captured_at": "2026-01-01T00:00:00Z",
            "system": {
                "status": "collected",
                "data": {
                    "os": "linux",
                    "arch": "x86_64",
                    "family": "unix",
                    "pointer_width": 64,
                    "endian": "little",
                    "container": "not_detected",
                    "wsl": "not_detected"
                }
            }
        }"#;
        let parsed = EnvironmentSnapshot::from_json_str(json).unwrap();
        let system = parsed.system.as_collected().expect("system");
        assert_eq!(system.os, "linux");
        assert!(parsed.system.issues().is_empty());
    }
}
