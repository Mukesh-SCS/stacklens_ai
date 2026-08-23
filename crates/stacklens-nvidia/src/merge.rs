use std::cmp::Ordering;

use stacklens_core::{GpuDevice, GpuInfo, NvidiaDriverInfo, Probe, ProbeIssue, ProbeIssueKind};

use crate::proc::{ProcCollection, ProcStatus};
use crate::smi::{SmiCollection, SmiStatus};
use crate::sysfs::{SysfsCollection, SysfsStatus};
use crate::NvidiaReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceStatus {
    Collected,
    NotPresent,
    NotInspected,
    Failed { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvidiaSources {
    pub sysfs: SourceStatus,
    pub proc_driver: SourceStatus,
    pub nvidia_smi: SourceStatus,
}

pub fn merge(sysfs: SysfsCollection, proc: ProcCollection, mut smi: SmiCollection) -> NvidiaReport {
    let sources = NvidiaSources {
        sysfs: sysfs_status(&sysfs),
        proc_driver: proc_status(&proc),
        nvidia_smi: smi_status(&smi),
    };

    let mut devices = sysfs.devices;
    for incoming in std::mem::take(&mut smi.devices) {
        merge_device(&mut devices, incoming);
    }
    sort_devices(&mut devices);

    let mut gpu_issues = Vec::new();
    gpu_issues.extend(sysfs.issues);
    gpu_issues.extend(smi.issues.iter().cloned());

    let gpu = build_gpu_probe(devices, gpu_issues, &sysfs.status, &smi.status);
    let driver = build_driver_probe(&proc, &smi, &gpu);

    NvidiaReport {
        gpu,
        driver,
        sources,
        driver_api_capability: smi.driver_api_cuda_version,
    }
}

fn sysfs_status(sysfs: &SysfsCollection) -> SourceStatus {
    match &sysfs.status {
        SysfsStatus::Collected => SourceStatus::Collected,
        SysfsStatus::NotPresent => SourceStatus::NotPresent,
        SysfsStatus::NotInspected => SourceStatus::NotInspected,
        SysfsStatus::Failed(issue) => SourceStatus::Failed {
            detail: issue.detail.clone(),
        },
    }
}

fn proc_status(proc: &ProcCollection) -> SourceStatus {
    match &proc.status {
        ProcStatus::Collected => SourceStatus::Collected,
        ProcStatus::NotPresent => SourceStatus::NotPresent,
        ProcStatus::NotInspected => SourceStatus::NotInspected,
        ProcStatus::Failed(issue) => SourceStatus::Failed {
            detail: issue.detail.clone(),
        },
    }
}

fn smi_status(smi: &SmiCollection) -> SourceStatus {
    match &smi.status {
        SmiStatus::Collected => SourceStatus::Collected,
        SmiStatus::NotPresent => SourceStatus::NotPresent,
        SmiStatus::Failed(issue) => SourceStatus::Failed {
            detail: issue.detail.clone(),
        },
    }
}

fn merge_device(devices: &mut Vec<GpuDevice>, incoming: GpuDevice) {
    if let Some(existing) = devices
        .iter_mut()
        .find(|device| same_device(device, &incoming))
    {
        fill_device(existing, incoming);
    } else {
        devices.push(incoming);
    }
}

fn same_device(left: &GpuDevice, right: &GpuDevice) -> bool {
    match (left.pci_bus_id.as_deref(), right.pci_bus_id.as_deref()) {
        (Some(left_pci), Some(right_pci)) if left_pci == right_pci => return true,
        _ => {}
    }
    match (left.uuid.as_deref(), right.uuid.as_deref()) {
        (Some(left_uuid), Some(right_uuid)) => left_uuid.eq_ignore_ascii_case(right_uuid),
        _ => false,
    }
}

fn fill_device(existing: &mut GpuDevice, incoming: GpuDevice) {
    if existing.index.is_none() {
        existing.index = incoming.index;
    }
    if existing.name.is_none() {
        existing.name = incoming.name;
    }
    if existing.uuid.is_none() {
        existing.uuid = incoming.uuid;
    }
    if existing.pci_bus_id.is_none() {
        existing.pci_bus_id = incoming.pci_bus_id;
    }
    if existing.pci_vendor_id.is_none() {
        existing.pci_vendor_id = incoming.pci_vendor_id;
    }
    if existing.pci_device_id.is_none() {
        existing.pci_device_id = incoming.pci_device_id;
    }
    if existing.cuda_compute_capability.is_none() {
        existing.cuda_compute_capability = incoming.cuda_compute_capability;
    }
    if existing.memory_bytes.is_none() {
        existing.memory_bytes = incoming.memory_bytes;
    }
}

pub fn sort_devices(devices: &mut [GpuDevice]) {
    devices.sort_by(|left, right| match (&left.pci_bus_id, &right.pci_bus_id) {
        (Some(left_pci), Some(right_pci)) => left_pci.cmp(right_pci),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => match (&left.uuid, &right.uuid) {
            (Some(left_uuid), Some(right_uuid)) => left_uuid
                .to_ascii_lowercase()
                .cmp(&right_uuid.to_ascii_lowercase()),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => left.index.cmp(&right.index),
        },
    });
}

fn build_gpu_probe(
    devices: Vec<GpuDevice>,
    issues: Vec<ProbeIssue>,
    sysfs: &SysfsStatus,
    smi: &SmiStatus,
) -> Probe<GpuInfo> {
    if !devices.is_empty() {
        return Probe::collected_with_issues(GpuInfo { devices }, issues);
    }

    let smi_failed = matches!(smi, SmiStatus::Failed(_));
    let sysfs_failed = matches!(sysfs, SysfsStatus::Failed(_));
    if smi_failed || sysfs_failed {
        if issues.is_empty() {
            return Probe::ProbeFailed {
                detail: "NVIDIA inventory collection failed".to_owned(),
            };
        }
        return Probe::collected_with_issues(
            GpuInfo {
                devices: Vec::new(),
            },
            issues,
        );
    }

    Probe::NotDetected
}

fn build_driver_probe(
    proc: &ProcCollection,
    smi: &SmiCollection,
    gpu: &Probe<GpuInfo>,
) -> Probe<NvidiaDriverInfo> {
    let mut issues = Vec::new();
    if let ProcStatus::Failed(issue) = &proc.status {
        issues.push(issue.clone());
    }

    match (&proc.version, &smi.driver_version) {
        (Some(kernel), Some(userspace)) if kernel == userspace => {
            return Probe::collected_with_issues(
                NvidiaDriverInfo {
                    version: Some(kernel.clone()),
                    kernel_module_version: Some(kernel.clone()),
                    userspace_version: Some(userspace.clone()),
                },
                issues,
            );
        }
        (Some(kernel), Some(userspace)) => {
            issues.push(ProbeIssue {
                source: "nvidia-driver".to_owned(),
                kind: ProbeIssueKind::Malformed,
                detail: format!(
                    "kernel module reports {kernel}; nvidia-smi/NVML userspace reports {userspace}"
                ),
            });
            return Probe::collected_with_issues(
                NvidiaDriverInfo {
                    version: None,
                    kernel_module_version: Some(kernel.clone()),
                    userspace_version: Some(userspace.clone()),
                },
                issues,
            );
        }
        (Some(kernel), None) => {
            return Probe::collected_with_issues(
                NvidiaDriverInfo {
                    version: Some(kernel.clone()),
                    kernel_module_version: Some(kernel.clone()),
                    userspace_version: None,
                },
                issues,
            );
        }
        (None, Some(userspace)) => {
            return Probe::collected_with_issues(
                NvidiaDriverInfo {
                    version: Some(userspace.clone()),
                    kernel_module_version: None,
                    userspace_version: Some(userspace.clone()),
                },
                issues,
            );
        }
        (None, None) => {}
    }

    if matches!(smi.status, SmiStatus::Failed(_)) && gpu.as_collected().is_some() {
        issues.extend(smi.issues.iter().cloned());
        return Probe::collected_with_issues(NvidiaDriverInfo::default(), issues);
    }

    if issues.is_empty() {
        Probe::NotDetected
    } else {
        Probe::collected_with_issues(NvidiaDriverInfo::default(), issues)
    }
}

#[cfg(test)]
mod tests {
    use super::{merge, sort_devices};
    use crate::proc::{ProcCollection, ProcStatus};
    use crate::smi::{SmiCollection, SmiStatus};
    use crate::sysfs::{SysfsCollection, SysfsStatus};
    use stacklens_core::{GpuDevice, GpuVendor, Probe, ProbeIssue, ProbeIssueKind};

    fn device(pci: &str, uuid: Option<&str>, name: Option<&str>) -> GpuDevice {
        GpuDevice {
            vendor: GpuVendor::Nvidia,
            index: None,
            name: name.map(ToOwned::to_owned),
            uuid: uuid.map(ToOwned::to_owned),
            pci_bus_id: Some(pci.to_owned()),
            pci_vendor_id: None,
            pci_device_id: None,
            cuda_compute_capability: None,
            memory_bytes: None,
        }
    }

    #[test]
    fn matches_by_pci_not_name() {
        let sysfs = SysfsCollection {
            devices: vec![device("00000000:01:00.0", None, None)],
            issues: Vec::new(),
            status: SysfsStatus::Collected,
        };
        let smi = SmiCollection {
            devices: vec![device(
                "00000000:01:00.0",
                Some("GPU-abc"),
                Some("NVIDIA GeForce RTX 4090"),
            )],
            driver_version: Some("580.82.07".to_owned()),
            driver_api_cuda_version: None,
            issues: Vec::new(),
            status: SmiStatus::Collected,
        };
        let report = merge(sysfs, ProcCollection::not_inspected(), smi);
        let devices = report.gpu.as_collected().unwrap().devices.clone();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].uuid.as_deref(), Some("GPU-abc"));
        assert_eq!(devices[0].name.as_deref(), Some("NVIDIA GeForce RTX 4090"));
    }

    #[test]
    fn matches_by_uuid_when_pci_missing() {
        let mut sysfs_dev = device("00000000:01:00.0", Some("GPU-abc"), None);
        sysfs_dev.pci_bus_id = None;
        let mut smi_dev = device("00000000:99:00.0", Some("GPU-abc"), Some("Name"));
        smi_dev.pci_bus_id = None;
        let sysfs = SysfsCollection {
            devices: vec![sysfs_dev],
            issues: Vec::new(),
            status: SysfsStatus::Collected,
        };
        let smi = SmiCollection {
            devices: vec![smi_dev],
            driver_version: None,
            driver_api_cuda_version: None,
            issues: Vec::new(),
            status: SmiStatus::Collected,
        };
        let report = merge(sysfs, ProcCollection::not_inspected(), smi);
        assert_eq!(report.gpu.as_collected().unwrap().devices.len(), 1);
    }

    #[test]
    fn does_not_merge_on_name_alone() {
        let sysfs = SysfsCollection {
            devices: vec![device("00000000:01:00.0", None, Some("Same Name"))],
            issues: Vec::new(),
            status: SysfsStatus::Collected,
        };
        let smi = SmiCollection {
            devices: vec![device("00000000:21:00.0", Some("GPU-b"), Some("Same Name"))],
            driver_version: None,
            driver_api_cuda_version: None,
            issues: Vec::new(),
            status: SmiStatus::Collected,
        };
        let report = merge(sysfs, ProcCollection::not_inspected(), smi);
        assert_eq!(report.gpu.as_collected().unwrap().devices.len(), 2);
    }

    #[test]
    fn sort_is_by_pci_then_uuid() {
        let mut devices = vec![
            device("00000000:21:00.0", Some("GPU-b"), None),
            device("00000000:01:00.0", Some("GPU-a"), None),
        ];
        sort_devices(&mut devices);
        assert_eq!(devices[0].pci_bus_id.as_deref(), Some("00000000:01:00.0"));
        assert_eq!(devices[1].pci_bus_id.as_deref(), Some("00000000:21:00.0"));
    }

    #[test]
    fn sysfs_devices_survive_smi_failure() {
        let sysfs = SysfsCollection {
            devices: vec![device("00000000:01:00.0", None, None)],
            issues: Vec::new(),
            status: SysfsStatus::Collected,
        };
        let issue = ProbeIssue {
            source: "nvidia-smi".to_owned(),
            kind: ProbeIssueKind::CommandFailed,
            detail: "Failed to initialize NVML: Driver/library version mismatch".to_owned(),
        };
        let smi = SmiCollection {
            devices: Vec::new(),
            driver_version: None,
            driver_api_cuda_version: None,
            issues: vec![issue.clone()],
            status: SmiStatus::Failed(issue),
        };
        let report = merge(sysfs, ProcCollection::not_inspected(), smi);
        assert_eq!(report.gpu.as_collected().unwrap().devices.len(), 1);
        assert_eq!(report.gpu.issues().len(), 1);
        assert!(matches!(report.gpu, Probe::Collected { .. }));
    }

    #[test]
    fn driver_mismatch_keeps_both_observations() {
        let proc = ProcCollection {
            version: Some("580.82.07".to_owned()),
            status: ProcStatus::Collected,
        };
        let smi = SmiCollection {
            devices: Vec::new(),
            driver_version: Some("570.124.04".to_owned()),
            driver_api_cuda_version: Some("13.0".to_owned()),
            issues: Vec::new(),
            status: SmiStatus::Collected,
        };
        let report = merge(
            SysfsCollection {
                devices: Vec::new(),
                issues: Vec::new(),
                status: SysfsStatus::Collected,
            },
            proc,
            smi,
        );
        let driver = report.driver.as_collected().unwrap();
        assert_eq!(driver.version, None);
        assert_eq!(driver.kernel_module_version.as_deref(), Some("580.82.07"));
        assert_eq!(driver.userspace_version.as_deref(), Some("570.124.04"));
        assert_eq!(report.driver_api_capability.as_deref(), Some("13.0"));
        assert!(report
            .driver
            .issues()
            .iter()
            .any(|issue| issue.detail.contains("570.124.04")));
    }
}
