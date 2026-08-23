use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use stacklens_core::{CudaVersion, NvccInfo, ProbeIssue, ProbeIssueKind};
use stacklens_system::process::{run_with_timeout, truncate_detail, MAX_CAPTURE_BYTES};

const NVCC_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NvccParse {
    pub compilation_tools_version: Option<CudaVersion>,
    pub raw_release: Option<String>,
    pub raw_build: Option<String>,
}

pub fn find_nvcc_on_path(entries: &[String]) -> Option<PathBuf> {
    for entry in entries {
        let candidate = Path::new(entry).join("nvcc");
        if is_nvcc_candidate(&candidate) {
            return Some(candidate);
        }
    }
    None
}

pub fn is_nvcc_candidate(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.is_file() || meta.file_type().is_symlink(),
        Err(_) => false,
    }
}

pub fn parse_nvcc_version(text: &str) -> NvccParse {
    let mut parsed = NvccParse {
        compilation_tools_version: None,
        raw_release: None,
        raw_build: None,
    };

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Cuda compilation tools, release ") {
            parsed.raw_release = Some(line.to_owned());
            if let Some((release, _)) = rest.split_once(',') {
                parsed.compilation_tools_version = CudaVersion::parse(release.trim());
            }
        } else if let Some(build) = line.strip_prefix("Build ") {
            parsed.raw_build = Some(build.trim().to_owned());
        }
    }

    parsed
}

pub fn run_nvcc_version(path: &Path) -> Result<String, ProbeIssue> {
    let mut command = Command::new(path);
    command.arg("--version");
    let outcome = run_with_timeout(command, NVCC_TIMEOUT, MAX_CAPTURE_BYTES);

    if outcome.not_found {
        return Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::CommandNotFound,
            detail: outcome
                .spawn_detail
                .unwrap_or_else(|| "nvcc was not found".to_owned()),
        });
    }
    if outcome.timed_out {
        return Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::Timeout,
            detail: format!("nvcc exceeded {}s timeout", NVCC_TIMEOUT.as_secs()),
        });
    }
    if let Some(detail) = outcome.spawn_detail {
        return Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::CommandFailed,
            detail,
        });
    }
    match outcome.exit_code {
        Some(0) => Ok(outcome.stdout),
        Some(code) => {
            let detail = if outcome.stderr.trim().is_empty() {
                format!("exit status {code}")
            } else {
                truncate_detail(&outcome.stderr)
            };
            Err(ProbeIssue {
                source: path.display().to_string(),
                kind: ProbeIssueKind::CommandFailed,
                detail,
            })
        }
        None => Err(ProbeIssue {
            source: path.display().to_string(),
            kind: ProbeIssueKind::CommandFailed,
            detail: "nvcc exited without a status".to_owned(),
        }),
    }
}

pub fn nvcc_info(path: PathBuf, parsed: NvccParse, associated_toolkit: Option<String>) -> NvccInfo {
    NvccInfo {
        path: path.display().to_string(),
        compilation_tools_version: parsed.compilation_tools_version,
        raw_release: parsed.raw_release,
        raw_build: parsed.raw_build,
        associated_toolkit,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_nvcc_version, NvccParse};

    fn fixture(name: &str) -> String {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/cuda")
            .join(name);
        std::fs::read_to_string(path).expect("nvcc fixture")
    }

    #[test]
    fn parses_realistic_12_8_output() {
        let parsed = parse_nvcc_version(&fixture("nvcc-12.8.txt"));
        assert_eq!(
            parsed
                .compilation_tools_version
                .as_ref()
                .map(|v| (v.major, v.minor)),
            Some((12, 8))
        );
        assert!(parsed
            .raw_release
            .as_deref()
            .unwrap_or("")
            .contains("release 12.8"));
        assert_eq!(
            parsed.raw_build.as_deref(),
            Some("cuda_12.8.r12.8/compiler.35583870_0")
        );
    }

    #[test]
    fn parses_realistic_13_0_output() {
        let parsed = parse_nvcc_version(&fixture("nvcc-13.0.txt"));
        assert_eq!(
            parsed
                .compilation_tools_version
                .as_ref()
                .map(|v| (v.major, v.minor)),
            Some((13, 0))
        );
    }

    #[test]
    fn malformed_output_does_not_panic() {
        let parsed = parse_nvcc_version(&fixture("nvcc-malformed.txt"));
        assert_eq!(
            parsed,
            NvccParse {
                compilation_tools_version: None,
                raw_release: None,
                raw_build: None,
            }
        );
    }

    #[test]
    fn empty_output_is_empty_parse() {
        let parsed = parse_nvcc_version("");
        assert!(parsed.compilation_tools_version.is_none());
    }
}
