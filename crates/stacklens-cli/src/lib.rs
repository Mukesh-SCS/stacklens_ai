use std::io::Write;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod capture;
mod diff;
mod doctor;
mod snapshot;

#[derive(Parser, Debug)]
#[command(
    name = "stacklens",
    version,
    about = "Inspect GPU and AI runtime environments and report evidence for configuration and compatibility problems.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Aggregate system diagnosis
    Doctor,
    /// Inspect NVIDIA GPU hardware and driver information [not implemented]
    Gpu,
    /// Inspect CUDA toolkit, runtime, and environment [not implemented]
    Cuda,
    /// Inspect native shared-library dependencies [not implemented]
    Libs {
        /// Shared library or executable to inspect
        path: Option<PathBuf>,
    },
    /// Inspect PyTorch / LibTorch installations [not implemented]
    Torch,
    /// Write a structured environment snapshot as JSON
    Snapshot,
    /// Compare two StackLens snapshots
    Diff {
        /// First snapshot (typically the known-good machine)
        left: PathBuf,
        /// Second snapshot
        right: PathBuf,
    },
}

pub fn execute(cli: Cli, stdout: &mut dyn Write) -> anyhow::Result<()> {
    match cli.command {
        Command::Doctor => doctor::run(stdout),
        Command::Gpu => not_implemented(
            stdout,
            "NVIDIA GPU and driver inspection is not implemented yet.",
        ),
        Command::Cuda => not_implemented(stdout, "CUDA toolkit inspection is not implemented yet."),
        Command::Libs { path } => libs(stdout, path.as_deref()),
        Command::Torch => not_implemented(
            stdout,
            "PyTorch / LibTorch inspection is not implemented yet.",
        ),
        Command::Snapshot => snapshot::run(stdout),
        Command::Diff { left, right } => diff::run(stdout, &left, &right),
    }
}

fn not_implemented(stdout: &mut dyn Write, message: &str) -> anyhow::Result<()> {
    writeln!(stdout, "{message}")?;
    Ok(())
}

fn libs(stdout: &mut dyn Write, path: Option<&std::path::Path>) -> anyhow::Result<()> {
    writeln!(
        stdout,
        "ELF shared-library inspection is not implemented yet."
    )?;
    if let Some(path) = path {
        writeln!(stdout, "Path received: {}", path.display())?;
        writeln!(stdout, "The file was not opened or parsed.")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{execute, Cli, Command};
    use clap::CommandFactory;
    use stacklens_core::EnvironmentSnapshot;

    #[test]
    fn clap_debug_assert() {
        Cli::command().debug_assert();
    }

    #[test]
    fn snapshot_json_is_schema_v1() {
        let mut buf = Vec::new();
        execute(
            Cli {
                command: Command::Snapshot,
            },
            &mut buf,
        )
        .unwrap();
        let snapshot =
            EnvironmentSnapshot::from_json_str(std::str::from_utf8(&buf).unwrap()).unwrap();
        assert_eq!(snapshot.schema_version, 1);
        assert!(matches!(
            snapshot.gpu,
            stacklens_core::Probe::NotImplemented
        ));
        assert!(snapshot.system.as_collected().is_some());
    }

    #[test]
    fn gpu_does_not_invent_devices() {
        let mut buf = Vec::new();
        execute(
            Cli {
                command: Command::Gpu,
            },
            &mut buf,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("not implemented"));
        assert!(!text.to_ascii_lowercase().contains("rtx"));
    }

    #[test]
    fn libs_acknowledges_path_without_reading() {
        let mut buf = Vec::new();
        execute(
            Cli {
                command: Command::Libs {
                    path: Some("/tmp/libtorch_cuda.so".into()),
                },
            },
            &mut buf,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("not implemented"));
        assert!(text.contains("/tmp/libtorch_cuda.so"));
        assert!(text.contains("was not opened"));
    }
}
