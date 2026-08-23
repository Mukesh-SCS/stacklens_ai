use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Endian {
    Little,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerDetection {
    NotInspected,
    NotDetected,
    Docker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WslDetection {
    NotInspected,
    NotDetected,
    Wsl1,
    Wsl2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub pointer_width: u32,
    pub endian: Endian,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_id_like: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_pretty_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_release: Option<String>,
    pub container: ContainerDetection,
    pub wsl: WslDetection,
}

/// Allowlisted diagnostic environment variables only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct EnvironmentInfo {
    pub variables: BTreeMap<String, String>,
}

/// GPU inventory. Fields stay optional until collectors can fill them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GpuInfo {
    pub devices: Vec<GpuDevice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GpuDevice {
    #[serde(default)]
    pub vendor: GpuVendor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pci_bus_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pci_vendor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pci_device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_compute_capability: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct NvidiaDriverInfo {
    /// Present when the kernel-module and userspace observations agree, or when
    /// only one source produced a version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_module_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userspace_version: Option<String>,
}

pub use crate::cuda::{
    CudaEnvironment, CudaInfo, CudaMajorFamilyEvidence, CudaRuntimeLibrary, CudaSelectors,
    CudaToolkit, CudaVersion, DriverApiCapability, ForwardCompatPackage, NvccInfo, PathSelector,
    RuntimeLibrarySource, ToolkitVersionSource,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct LibraryCandidate {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soname: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct LibraryInfo {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    pub needed: Vec<String>,
    pub rpath: Vec<String>,
    pub runpath: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soname: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct TorchInfo {
    pub install_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda_version: Option<String>,
    pub libraries: Vec<LibraryCandidate>,
}
