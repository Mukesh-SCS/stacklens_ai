use std::fs;
use std::path::Path;

use stacklens_core::{ProbeIssue, ProbeIssueKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcStatus {
    Collected,
    NotPresent,
    NotInspected,
    Failed(ProbeIssue),
}

#[derive(Debug, Clone)]
pub struct ProcCollection {
    pub version: Option<String>,
    pub status: ProcStatus,
}

impl ProcCollection {
    pub fn not_inspected() -> Self {
        Self {
            version: None,
            status: ProcStatus::NotInspected,
        }
    }
}

pub fn collect_proc_driver(path: &Path) -> ProcCollection {
    match fs::read_to_string(path) {
        Ok(text) => match parse_driver_version(&text) {
            Some(version) => ProcCollection {
                version: Some(version),
                status: ProcStatus::Collected,
            },
            None => ProcCollection {
                version: None,
                status: ProcStatus::Failed(ProbeIssue {
                    source: path.display().to_string(),
                    kind: ProbeIssueKind::Malformed,
                    detail: "NVRM version line did not contain a recognizable driver version"
                        .to_owned(),
                }),
            },
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ProcCollection {
            version: None,
            status: ProcStatus::NotPresent,
        },
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => ProcCollection {
            version: None,
            status: ProcStatus::Failed(ProbeIssue {
                source: path.display().to_string(),
                kind: ProbeIssueKind::PermissionDenied,
                detail: err.to_string(),
            }),
        },
        Err(err) => ProcCollection {
            version: None,
            status: ProcStatus::Failed(ProbeIssue {
                source: path.display().to_string(),
                kind: ProbeIssueKind::CommandFailed,
                detail: err.to_string(),
            }),
        },
    }
}

pub fn parse_driver_version(text: &str) -> Option<String> {
    let line = text
        .lines()
        .find(|line| line.to_ascii_uppercase().contains("NVRM VERSION"))?;
    line.split_whitespace().find_map(looks_like_version)
}

fn looks_like_version(token: &str) -> Option<String> {
    let bytes = token.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return None;
    }
    let mut dots = 0u32;
    for b in bytes {
        match b {
            b'0'..=b'9' => {}
            b'.' => dots += 1,
            _ => return None,
        }
    }
    if dots >= 1 {
        Some(token.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::parse_driver_version;

    #[test]
    fn closed_kernel_module_line() {
        let text = "NVRM version: NVIDIA UNIX x86_64 Kernel Module  580.82.07  Tue Aug 5 12:00:00 UTC 2025\nGCC version: gcc version 13.2.0\n";
        assert_eq!(parse_driver_version(text).as_deref(), Some("580.82.07"));
    }

    #[test]
    fn open_kernel_module_line() {
        let text =
            "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  560.35.03  Release Build\n";
        assert_eq!(parse_driver_version(text).as_deref(), Some("560.35.03"));
    }

    #[test]
    fn missing_or_garbage() {
        assert_eq!(parse_driver_version("GCC version: 13.2.0"), None);
        assert_eq!(parse_driver_version(""), None);
        assert_eq!(
            parse_driver_version("NVRM version: NVIDIA UNIX x86_64 Kernel Module"),
            None
        );
    }
}
