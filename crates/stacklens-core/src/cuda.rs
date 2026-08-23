use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

/// Parsed CUDA version. Components are integers; `12.10` is not `12.1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CudaVersion {
    pub major: u16,
    pub minor: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<u16>,
    pub raw: String,
}

impl CudaVersion {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut parts = trimmed.split('.');
        let major = parse_version_component(parts.next()?)?;
        let minor = parse_version_component(parts.next()?)?;
        let patch = match parts.next() {
            Some(component) => Some(parse_version_component(component)?),
            None => None,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
            raw: trimmed.to_owned(),
        })
    }

    fn cmp_key(&self) -> (u16, u16, u16) {
        (self.major, self.minor, self.patch.unwrap_or(0))
    }
}

fn parse_version_component(raw: &str) -> Option<u16> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok()
}

impl PartialEq for CudaVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp_key() == other.cmp_key()
    }
}

impl Eq for CudaVersion {}

impl PartialOrd for CudaVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CudaVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_key().cmp(&other.cmp_key())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolkitVersionSource {
    ToolkitMetadata,
    VersionTxt,
    Nvcc,
    DirectoryName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeLibrarySource {
    Toolkit,
    LdLibraryPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct CudaInfo {
    pub toolkits: Vec<CudaToolkit>,
    pub selectors: CudaSelectors,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvcc: Option<NvccInfo>,
    pub runtime_libraries: Vec<CudaRuntimeLibrary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_api_capability: Option<DriverApiCapability>,
    pub forward_compat: Vec<ForwardCompatPackage>,
    pub environment: CudaEnvironment,
    pub compatibility: Vec<CudaMajorFamilyEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_context: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct CudaToolkit {
    pub path: String,
    pub is_symlink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<CudaVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_source: Option<ToolkitVersionSource>,
    pub nvcc_present: bool,
    pub libcudart_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct CudaSelectors {
    pub convenience_paths: Vec<PathSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_home: Option<PathSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_path: Option<PathSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_root: Option<PathSelector>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_nvcc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct PathSelector {
    pub path: String,
    pub is_symlink: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NvccInfo {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compilation_tools_version: Option<CudaVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_release: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub associated_toolkit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CudaRuntimeLibrary {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
    pub source: RuntimeLibrarySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DriverApiCapability {
    pub raw: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<CudaVersion>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct ForwardCompatPackage {
    pub path: String,
    pub libraries: Vec<String>,
    pub referenced_by_ld_library_path: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct CudaEnvironment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_home: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_root: Option<String>,
    pub path_entries: Vec<String>,
    pub ld_library_path_entries: Vec<String>,
}

/// Neutral evidence only. Meeting a documented driver-branch minimum is not
/// a determination that the system is fully compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CudaMajorFamilyEvidence {
    pub cuda_major: u16,
    pub documented_minimum_driver_branch: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_driver_branch: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub driver_branch_meets_documented_minimum: Option<bool>,
    pub source: String,
    pub note: String,
}

#[cfg(test)]
mod tests {
    use super::CudaVersion;

    #[test]
    fn parses_two_and_three_component_versions() {
        let v118 = CudaVersion::parse("11.8").unwrap();
        assert_eq!(v118.major, 11);
        assert_eq!(v118.minor, 8);
        assert_eq!(v118.patch, None);

        let v124 = CudaVersion::parse("12.4").unwrap();
        assert_eq!((v124.major, v124.minor), (12, 4));

        let v128 = CudaVersion::parse("12.8").unwrap();
        let v1281 = CudaVersion::parse("12.8.1").unwrap();
        assert_eq!(v128.minor, 8);
        assert_eq!(v1281.patch, Some(1));
        assert!(v1281 > v128);

        let v130 = CudaVersion::parse("13.0").unwrap();
        let v132 = CudaVersion::parse("13.2").unwrap();
        assert!(v132 > v130);
    }

    #[test]
    fn twelve_ten_is_not_twelve_one() {
        let a = CudaVersion::parse("12.10").unwrap();
        let b = CudaVersion::parse("12.1").unwrap();
        assert_ne!(a, b);
        assert!(a > b);
        assert_eq!(a.minor, 10);
        assert_eq!(b.minor, 1);
    }

    #[test]
    fn omitted_patch_compares_as_zero() {
        let a = CudaVersion::parse("12.8").unwrap();
        let b = CudaVersion::parse("12.8.0").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
    }

    #[test]
    fn malformed_versions_are_none() {
        assert!(CudaVersion::parse("").is_none());
        assert!(CudaVersion::parse("12").is_none());
        assert!(CudaVersion::parse("12.x").is_none());
        assert!(CudaVersion::parse("cuda-12.8").is_none());
        assert!(CudaVersion::parse("12.8.1.2").is_none());
        assert!(CudaVersion::parse("12.8-rc").is_none());
    }

    #[test]
    fn retains_raw_text() {
        let version = CudaVersion::parse(" 12.8.1 ").unwrap();
        assert_eq!(version.raw, "12.8.1");
    }
}
