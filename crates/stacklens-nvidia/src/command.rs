use std::process::Command;
use std::time::Duration;

pub use stacklens_system::process::{
    run_with_timeout, truncate_detail, CommandOutcome, MAX_CAPTURE_BYTES,
};

pub const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(8);

pub fn run_nvidia_smi_query(fields: &str) -> CommandOutcome {
    let mut command = Command::new("nvidia-smi");
    command.arg(format!("--query-gpu={fields}"));
    command.arg("--format=csv,noheader,nounits");
    run_with_timeout(command, NVIDIA_SMI_TIMEOUT, MAX_CAPTURE_BYTES)
}
