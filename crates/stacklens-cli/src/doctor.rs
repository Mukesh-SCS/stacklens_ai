use std::io::Write;

use stacklens_core::{ContainerDetection, WslDetection};

use crate::capture::current_snapshot;

pub fn run(stdout: &mut dyn Write) -> anyhow::Result<()> {
    let snapshot = current_snapshot();

    writeln!(stdout, "StackLens doctor")?;
    writeln!(stdout, "Status: early development (Phase 1C)")?;
    writeln!(stdout)?;

    match snapshot.system.as_collected() {
        Some(system) => {
            writeln!(stdout, "System")?;
            writeln!(stdout, "  os: {}", system.os)?;
            writeln!(stdout, "  arch: {}", system.arch)?;
            writeln!(stdout, "  family: {}", system.family)?;
            writeln!(stdout, "  pointer_width: {}", system.pointer_width)?;
            writeln!(
                stdout,
                "  endian: {}",
                match system.endian {
                    stacklens_core::Endian::Little => "little",
                    stacklens_core::Endian::Big => "big",
                }
            )?;
            write_optional(stdout, "os_id", system.os_id.as_deref())?;
            write_optional(stdout, "os_pretty_name", system.os_pretty_name.as_deref())?;
            write_optional(stdout, "os_version_id", system.os_version_id.as_deref())?;
            write_optional(stdout, "kernel_release", system.kernel_release.as_deref())?;
            writeln!(
                stdout,
                "  container: {}",
                format_container(&system.container)
            )?;
            writeln!(stdout, "  wsl: {}", format_wsl(&system.wsl))?;
        }
        None => writeln!(stdout, "System: not collected")?,
    }

    writeln!(stdout)?;
    writeln!(stdout, "Environment (allowlisted)")?;
    match snapshot.environment.as_collected() {
        Some(env) if env.variables.is_empty() => {
            writeln!(stdout, "  none of the diagnostic variables are set")?;
        }
        Some(env) => {
            for (key, value) in &env.variables {
                writeln!(stdout, "  {key}={value}")?;
            }
        }
        None => writeln!(stdout, "  not collected")?,
    }

    writeln!(stdout)?;
    writeln!(stdout, "GPU")?;
    match snapshot.gpu.as_collected() {
        Some(info) if info.devices.is_empty() => {
            writeln!(stdout, "  no device inventory")?;
        }
        Some(info) => {
            for (index, device) in info.devices.iter().enumerate() {
                writeln!(
                    stdout,
                    "  [{}] {}  pci={}  uuid={}",
                    device.index.unwrap_or(index as u32),
                    device.name.as_deref().unwrap_or("(name unavailable)"),
                    device.pci_bus_id.as_deref().unwrap_or("?"),
                    device.uuid.as_deref().unwrap_or("?")
                )?;
            }
        }
        None => writeln!(stdout, "  not detected")?,
    }
    for issue in snapshot.gpu.issues() {
        writeln!(stdout, "  issue: {} ({})", issue.detail, issue.source)?;
    }

    writeln!(stdout)?;
    writeln!(stdout, "NVIDIA driver")?;
    match snapshot.nvidia_driver.as_collected() {
        Some(info) => {
            if let Some(version) = info.version.as_deref() {
                writeln!(stdout, "  version: {version}")?;
            } else {
                writeln!(stdout, "  version: (sources disagree)")?;
            }
            if let Some(kernel) = info.kernel_module_version.as_deref() {
                writeln!(stdout, "  kernel_module: {kernel}")?;
            }
            if let Some(userspace) = info.userspace_version.as_deref() {
                writeln!(stdout, "  userspace: {userspace}")?;
            }
        }
        None => writeln!(stdout, "  not detected")?,
    }
    for issue in snapshot.nvidia_driver.issues() {
        writeln!(stdout, "  issue: {} ({})", issue.detail, issue.source)?;
    }

    writeln!(stdout)?;
    crate::cuda::write_doctor_summary(stdout, &snapshot.cuda)?;

    writeln!(stdout)?;
    writeln!(stdout, "Not yet implemented")?;
    writeln!(stdout, "  libs, torch, diagnostic rules")?;
    writeln!(stdout)?;
    writeln!(stdout, "Findings")?;
    writeln!(stdout, "  none")?;
    writeln!(stdout, "Diagnostic rules: not implemented")?;
    Ok(())
}

fn write_optional(stdout: &mut dyn Write, name: &str, value: Option<&str>) -> anyhow::Result<()> {
    match value {
        Some(value) => writeln!(stdout, "  {name}: {value}")?,
        None => writeln!(stdout, "  {name}: (not available)")?,
    }
    Ok(())
}

fn format_container(value: &ContainerDetection) -> &'static str {
    match value {
        ContainerDetection::NotInspected => "not inspected",
        ContainerDetection::NotDetected => "not detected",
        ContainerDetection::Docker => "docker",
    }
}

fn format_wsl(value: &WslDetection) -> &'static str {
    match value {
        WslDetection::NotInspected => "not inspected",
        WslDetection::NotDetected => "not detected",
        WslDetection::Wsl1 => "wsl1",
        WslDetection::Wsl2 => "wsl2",
    }
}
