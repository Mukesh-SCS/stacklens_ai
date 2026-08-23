use serde::{Deserialize, Serialize};

/// Outcome of attempting to collect a piece of the environment.
///
/// Absence of NVIDIA software, CUDA, or PyTorch is a probe state, not an
/// application error.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Probe<T> {
    Collected {
        data: T,
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

impl<T> Probe<T> {
    pub fn collected(data: T) -> Self {
        Self::Collected { data }
    }

    pub fn as_collected(&self) -> Option<&T> {
        match self {
            Self::Collected { data } => Some(data),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Probe;
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
}
