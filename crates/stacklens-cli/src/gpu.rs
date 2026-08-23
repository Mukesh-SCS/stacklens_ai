use std::io::Write;

use stacklens_core::{GpuDevice, GpuInfo, NvidiaDriverInfo, Probe, ProbeIssueKind};
use stacklens_nvidia::{NvidiaReport, SourceStatus};

pub fn run(stdout: &mut dyn Write) -> anyhow::Result<()> {
    render(stdout, &stacklens_nvidia::inspect())
}

pub fn render(stdout: &mut dyn Write, report: &NvidiaReport) -> anyhow::Result<()> {
    match report.gpu.as_collected() {
        Some(info) if !info.devices.is_empty() => {
            writeln!(stdout, "NVIDIA")?;
            writeln!(stdout)?;
            write_driver(stdout, &report.driver)?;
            writeln!(stdout)?;
            write_devices(stdout, info)?;
        }
        Some(_) => writeln!(stdout, "No NVIDIA GPU inventory collected.")?,
        None => writeln!(stdout, "No NVIDIA GPU detected.")?,
    }

    writeln!(stdout)?;
    writeln!(stdout, "Sources")?;
    writeln!(stdout, "  sysfs: {}", format_source(&report.sources.sysfs))?;
    writeln!(
        stdout,
        "  /proc/driver/nvidia/version: {}",
        format_source(&report.sources.proc_driver)
    )?;
    writeln!(
        stdout,
        "  nvidia-smi: {}",
        format_source(&report.sources.nvidia_smi)
    )?;

    let issues: Vec<_> = report
        .gpu
        .issues()
        .iter()
        .chain(report.driver.issues())
        .collect();
    if !issues.is_empty() {
        writeln!(stdout)?;
        writeln!(stdout, "Issues")?;
        for issue in issues {
            writeln!(
                stdout,
                "  {}: {}: {}",
                issue.source,
                format_kind(issue.kind),
                issue.detail
            )?;
        }
    }

    Ok(())
}

fn write_driver(stdout: &mut dyn Write, driver: &Probe<NvidiaDriverInfo>) -> anyhow::Result<()> {
    writeln!(stdout, "Driver")?;
    match driver.as_collected() {
        Some(info) => {
            match info.version.as_deref() {
                Some(version) => writeln!(stdout, "  version: {version}")?,
                None => writeln!(stdout, "  version: (sources disagree)")?,
            }
            if info.kernel_module_version.is_some() || info.userspace_version.is_some() {
                writeln!(
                    stdout,
                    "  kernel_module: {}",
                    info.kernel_module_version
                        .as_deref()
                        .unwrap_or("(not available)")
                )?;
                writeln!(
                    stdout,
                    "  userspace: {}",
                    info.userspace_version
                        .as_deref()
                        .unwrap_or("(not available)")
                )?;
            }
        }
        None => writeln!(stdout, "  version: (not available)")?,
    }
    Ok(())
}

fn write_devices(stdout: &mut dyn Write, info: &GpuInfo) -> anyhow::Result<()> {
    for (i, device) in info.devices.iter().enumerate() {
        writeln!(stdout, "GPU {i}")?;
        write_device_fields(stdout, device)?;
        if i + 1 != info.devices.len() {
            writeln!(stdout)?;
        }
    }
    Ok(())
}

fn write_device_fields(stdout: &mut dyn Write, device: &GpuDevice) -> anyhow::Result<()> {
    writeln!(
        stdout,
        "  name: {}",
        device.name.as_deref().unwrap_or("(not available)")
    )?;
    writeln!(
        stdout,
        "  uuid: {}",
        device.uuid.as_deref().unwrap_or("(not available)")
    )?;
    writeln!(
        stdout,
        "  pci_bus_id: {}",
        device.pci_bus_id.as_deref().unwrap_or("(not available)")
    )?;
    writeln!(stdout, "  memory: {}", format_memory(device.memory_bytes))?;
    writeln!(
        stdout,
        "  cuda_compute_capability: {}",
        device
            .cuda_compute_capability
            .as_deref()
            .unwrap_or("unavailable")
    )?;
    Ok(())
}

fn format_memory(bytes: Option<u64>) -> String {
    match bytes {
        Some(bytes) if bytes % (1024 * 1024) == 0 => format!("{} MiB", bytes / (1024 * 1024)),
        Some(bytes) => format!("{bytes} bytes"),
        None => "(not available)".to_owned(),
    }
}

fn format_source(status: &SourceStatus) -> String {
    match status {
        SourceStatus::Collected => "collected".to_owned(),
        SourceStatus::NotPresent => "not present".to_owned(),
        SourceStatus::NotInspected => "not inspected".to_owned(),
        SourceStatus::Failed { detail } => format!("failed ({detail})"),
    }
}

fn format_kind(kind: ProbeIssueKind) -> &'static str {
    match kind {
        ProbeIssueKind::PermissionDenied => "permission_denied",
        ProbeIssueKind::CommandFailed => "command_failed",
        ProbeIssueKind::CommandNotFound => "command_not_found",
        ProbeIssueKind::Malformed => "malformed",
        ProbeIssueKind::Unsupported => "unsupported",
        ProbeIssueKind::Timeout => "timeout",
    }
}
