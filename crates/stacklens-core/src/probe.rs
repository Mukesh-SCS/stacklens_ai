use serde::{Deserialize, Serialize};

/// Outcome of attempting to collect a piece of the environment.
///
/// Absence of NVIDIA software, CUDA, or PyTorch is a probe state, not an
/// application error. A collected probe may still carry source-level issues.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Probe<T> {
    Collected {
        data: T,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        issues: Vec<ProbeIssue>,
    },
    #[default]
    NotImplemented,
    NotDetected,
    NotInstalled,
    PermissionDenied {
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    Unsupported {
        detail: String,
    },
    ProbeFailed {
        detail: String,
    },
    Malformed {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProbeIssue {
    pub source: String,
    pub kind: ProbeIssueKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeIssueKind {
    PermissionDenied,
    CommandFailed,
    CommandNotFound,
    Malformed,
    Unsupported,
    Timeout,
}

impl<T> Probe<T> {
    pub fn collected(data: T) -> Self {
        Self::Collected {
            data,
            issues: Vec::new(),
        }
    }

    pub fn collected_with_issues(data: T, issues: Vec<ProbeIssue>) -> Self {
        Self::Collected { data, issues }
    }

    pub fn as_collected(&self) -> Option<&T> {
        match self {
            Self::Collected { data, .. } => Some(data),
            _ => None,
        }
    }

    pub fn issues(&self) -> &[ProbeIssue] {
        match self {
            Self::Collected { issues, .. } => issues,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Probe, ProbeIssue, ProbeIssueKind};
    use serde_json::json;

    #[test]
    fn collected_roundtrip() {
        let probe = Probe::collected("x86_64".to_owned());
        let value = serde_json::to_value(&probe).unwrap();
        assert_eq!(value, json!({"status": "collected", "data": "x86_64"}));
        let parsed: Probe<String> = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, probe);
    }

    #[test]
    fn not_implemented_is_unit_status() {
        let value = serde_json::to_value(Probe::<()>::NotImplemented).unwrap();
        assert_eq!(value, json!({"status": "not_implemented"}));
    }

    #[test]
    fn collected_without_issues_field_still_deserializes() {
        let parsed: Probe<String> =
            serde_json::from_value(json!({"status": "collected", "data": "ok"})).unwrap();
        assert_eq!(parsed.as_collected().map(String::as_str), Some("ok"));
        assert!(parsed.issues().is_empty());
    }

    #[test]
    fn collected_with_issues_roundtrip() {
        let probe = Probe::collected_with_issues(
            "partial".to_owned(),
            vec![ProbeIssue {
                source: "nvidia-smi".to_owned(),
                kind: ProbeIssueKind::CommandFailed,
                detail: "Failed to initialize NVML: Driver/library version mismatch".to_owned(),
            }],
        );
        let value = serde_json::to_value(&probe).unwrap();
        assert_eq!(value["issues"][0]["kind"], "command_failed");
        let parsed: Probe<String> = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, probe);
    }
}
