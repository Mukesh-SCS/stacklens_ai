use std::io::Write;

use stacklens_core::{ContainerDetection, WslDetection};

use crate::capture::current_snapshot;

pub fn run(stdout: &mut dyn Write) -> anyhow::Result<()> {
    let snapshot = current_snapshot();

    writeln!(stdout, "StackLens doctor")?;
    writeln!(stdout, "Status: early development (Phase 1A)")?;
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
    writeln!(stdout, "Not yet implemented")?;
    writeln!(stdout, "  gpu, cuda, libs, torch, diagnostic rules")?;
    writeln!(stdout)?;
    writeln!(stdout, "Findings")?;
    writeln!(stdout, "  none")?;
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
