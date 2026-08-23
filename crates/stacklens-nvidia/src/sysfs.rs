use std::fs;
use std::path::Path;

use stacklens_core::{GpuDevice, GpuVendor, ProbeIssue, ProbeIssueKind};

use crate::pci::normalize_pci_bus_id;

const NVIDIA_VENDOR_ID: u32 = 0x10de;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SysfsStatus {
    Collected,
    NotPresent,
    NotInspected,
    Failed(ProbeIssue),
}

#[derive(Debug, Clone)]
pub struct SysfsCollection {
    pub devices: Vec<GpuDevice>,
    pub issues: Vec<ProbeIssue>,
    pub status: SysfsStatus,
}

impl SysfsCollection {
    pub fn not_inspected() -> Self {
        Self {
            devices: Vec::new(),
            issues: Vec::new(),
            status: SysfsStatus::NotInspected,
        }
    }
}

pub fn collect_from_sysfs(root: &Path) -> SysfsCollection {
    let devices_dir = root.join("bus/pci/devices");
    let entries = match fs::read_dir(&devices_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return SysfsCollection {
                devices: Vec::new(),
                issues: Vec::new(),
                status: SysfsStatus::NotPresent,
            };
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            let issue = ProbeIssue {
                source: devices_dir.display().to_string(),
                kind: ProbeIssueKind::PermissionDenied,
                detail: err.to_string(),
            };
            return SysfsCollection {
                devices: Vec::new(),
                issues: vec![issue.clone()],
                status: SysfsStatus::Failed(issue),
            };
        }
        Err(err) => {
            let issue = ProbeIssue {
                source: devices_dir.display().to_string(),
                kind: ProbeIssueKind::CommandFailed,
                detail: err.to_string(),
            };
            return SysfsCollection {
                devices: Vec::new(),
                issues: vec![issue.clone()],
                status: SysfsStatus::Failed(issue),
            };
        }
    };

    let mut devices = Vec::new();
    let mut issues = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                issues.push(ProbeIssue {
                    source: devices_dir.display().to_string(),
                    kind: ProbeIssueKind::PermissionDenied,
                    detail: err.to_string(),
                });
                continue;
            }
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        match read_pci_gpu(&path) {
            Ok(Some(device)) => devices.push(device),
            Ok(None) => {}
            Err(issue) => issues.push(issue),
        }
    }

    SysfsCollection {
        devices,
        issues,
        status: SysfsStatus::Collected,
    }
}

fn read_pci_gpu(dir: &Path) -> Result<Option<GpuDevice>, ProbeIssue> {
    let vendor_raw = read_sysfs_value(dir, "vendor")?;
    let class_raw = read_sysfs_value(dir, "class")?;
    let vendor_id = parse_hex_id(&vendor_raw).ok_or_else(|| ProbeIssue {
        source: dir.join("vendor").display().to_string(),
        kind: ProbeIssueKind::Malformed,
        detail: format!("unrecognized PCI vendor value: {vendor_raw}"),
    })?;
    let class = parse_hex_id(&class_raw).ok_or_else(|| ProbeIssue {
        source: dir.join("class").display().to_string(),
        kind: ProbeIssueKind::Malformed,
        detail: format!("unrecognized PCI class value: {class_raw}"),
    })?;

    if vendor_id != NVIDIA_VENDOR_ID || !is_display_or_3d_class(class) {
        return Ok(None);
    }

    let device_id = match read_sysfs_value(dir, "device") {
        Ok(raw) => parse_hex_id(&raw).map(|id| format!("{id:04x}")),
        Err(_) => None,
    };

    let pci_bus_id = pci_address_from_dir(dir);

    Ok(Some(GpuDevice {
        vendor: GpuVendor::Nvidia,
        index: None,
        name: None,
        uuid: None,
        pci_bus_id,
        pci_vendor_id: Some(format!("{vendor_id:04x}")),
        pci_device_id: device_id,
        cuda_compute_capability: None,
        memory_bytes: None,
    }))
}

fn pci_address_from_dir(dir: &Path) -> Option<String> {
    if let Some(from_name) = dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(normalize_pci_bus_id)
    {
        return Some(from_name);
    }
    // Fixtures on Windows cannot use `:` in directory names, so they may
    // supply an `address` file. Live Linux sysfs BDFs never take this path.
    fs::read_to_string(dir.join("address"))
        .ok()
        .and_then(|text| normalize_pci_bus_id(text.trim()))
}

fn read_sysfs_value(dir: &Path, name: &str) -> Result<String, ProbeIssue> {
    let path = dir.join(name);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(text.trim().to_owned()),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::PermissionDenied,
            detail: err.to_string(),
        }),
        Err(err) => Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::CommandFailed,
            detail: err.to_string(),
        }),
    }
}

fn parse_hex_id(raw: &str) -> Option<u32> {
    let value = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(value, 16).ok()
}

fn is_display_or_3d_class(class: u32) -> bool {
    let base_class = (class >> 16) & 0xff;
    let subclass = (class >> 8) & 0xff;
    base_class == 0x03 && (subclass == 0x00 || subclass == 0x02)
}

#[cfg(test)]
mod tests {
    use super::{collect_from_sysfs, is_display_or_3d_class};
    use stacklens_core::GpuVendor;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/nvidia/sysfs")
            .join(name)
    }

    #[test]
    fn class_filter() {
        assert!(is_display_or_3d_class(0x030000));
        assert!(is_display_or_3d_class(0x030200));
        assert!(!is_display_or_3d_class(0x040300));
        assert!(!is_display_or_3d_class(0x020000));
        assert!(
            !is_display_or_3d_class(0x0300),
            "must not treat a truncated 0x0300 value as VGA (03:00)"
        );
    }

    #[test]
    fn class_from_sysfs_text() {
        use super::parse_hex_id;
        let vga = parse_hex_id("0x030000").expect("vga");
        let three_d = parse_hex_id("0x030200").expect("3d");
        let audio = parse_hex_id("0x040300").expect("audio");
        assert_eq!((vga >> 16) & 0xff, 0x03);
        assert_eq!((vga >> 8) & 0xff, 0x00);
        assert_eq!((three_d >> 16) & 0xff, 0x03);
        assert_eq!((three_d >> 8) & 0xff, 0x02);
        assert!(is_display_or_3d_class(vga));
        assert!(is_display_or_3d_class(three_d));
        assert!(!is_display_or_3d_class(audio));
    }

    #[test]
    fn single_gpu() {
        let collected = collect_from_sysfs(&fixture("single-gpu"));
        assert_eq!(collected.devices.len(), 1);
        assert_eq!(collected.devices[0].vendor, GpuVendor::Nvidia);
        assert_eq!(
            collected.devices[0].pci_bus_id.as_deref(),
            Some("00000000:01:00.0")
        );
        assert_eq!(collected.devices[0].pci_device_id.as_deref(), Some("2684"));
        assert!(collected.devices[0].name.is_none());
    }

    #[test]
    fn multiple_gpus() {
        let collected = collect_from_sysfs(&fixture("multi-gpu"));
        assert_eq!(collected.devices.len(), 2);
    }

    #[test]
    fn ignores_non_nvidia_display() {
        let collected = collect_from_sysfs(&fixture("non-nvidia"));
        assert!(collected.devices.is_empty());
        assert!(collected.issues.is_empty());
    }

    #[test]
    fn ignores_nvidia_audio() {
        let collected = collect_from_sysfs(&fixture("nvidia-audio"));
        assert!(collected.devices.is_empty());
        assert!(collected.issues.is_empty());
    }

    #[test]
    fn malformed_vendor_is_an_issue() {
        let collected = collect_from_sysfs(&fixture("malformed"));
        assert!(collected.devices.is_empty());
        assert!(collected
            .issues
            .iter()
            .any(|issue| issue.detail.contains("unrecognized PCI vendor")));
    }

    #[test]
    fn missing_tree_is_not_present() {
        let collected = collect_from_sysfs(&fixture("does-not-exist"));
        assert!(matches!(collected.status, super::SysfsStatus::NotPresent));
    }
}
