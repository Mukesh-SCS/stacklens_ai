mod command;
mod merge;
mod pci;
mod proc;
mod smi;
mod sysfs;

use std::path::Path;

use stacklens_core::{GpuInfo, NvidiaDriverInfo, Probe};

pub use merge::{NvidiaSources, SourceStatus};
pub use pci::normalize_pci_bus_id;
pub use proc::parse_driver_version;
pub use smi::{parse_compute_cap_csv, parse_smi_inventory_csv};
pub use sysfs::collect_from_sysfs;

pub struct NvidiaReport {
    pub gpu: Probe<GpuInfo>,
    pub driver: Probe<NvidiaDriverInfo>,
    pub sources: NvidiaSources,
    /// Driver-side CUDA API capability from NVIDIA tooling, not a toolkit version.
    pub driver_api_capability: Option<String>,
}

pub fn inspect() -> NvidiaReport {
    let sysfs = if cfg!(target_os = "linux") {
        collect_from_sysfs(Path::new("/sys"))
    } else {
        sysfs::SysfsCollection::not_inspected()
    };

    let proc = if cfg!(target_os = "linux") {
        proc::collect_proc_driver(Path::new("/proc/driver/nvidia/version"))
    } else {
        proc::ProcCollection::not_inspected()
    };

    let smi = smi::collect_nvidia_smi();
    merge::merge(sysfs, proc, smi)
}

#[cfg(test)]
mod tests {
    use super::inspect;
    use stacklens_core::Probe;

    #[test]
    fn inspect_does_not_require_nvidia_hardware() {
        let report = inspect();
        match &report.gpu {
            Probe::NotDetected | Probe::Collected { .. } | Probe::ProbeFailed { .. } => {}
            other => panic!("unexpected gpu probe: {other:?}"),
        }
    }
}
