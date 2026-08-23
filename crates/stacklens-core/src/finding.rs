use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct DiagnosticEvidence {
    pub facts: Vec<String>,
}

/// A rule conclusion. Evidence and finding text are stored separately on
/// purpose: renderers must not collapse them into a single guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DiagnosticFinding {
    pub id: String,
    pub severity: Severity,
    pub title: String,
    pub finding: String,
    pub evidence: DiagnosticEvidence,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
    pub affected: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticEvidence, DiagnosticFinding, Severity};

    #[test]
    fn finding_roundtrip_keeps_evidence_separate() {
        let finding = DiagnosticFinding {
            id: "ELF-LIB-0002".to_owned(),
            severity: Severity::High,
            title: "Multiple conflicting library candidates".to_owned(),
            finding: "More than one CUDA runtime candidate exists.".to_owned(),
            evidence: DiagnosticEvidence {
                facts: vec![
                    "libtorch_cuda.so requires libcudart.so.12".to_owned(),
                    "loader candidate is /usr/local/cuda/lib64/libcudart.so.12".to_owned(),
                ],
            },
            explanation: None,
            affected: vec!["cuda".to_owned(), "pytorch".to_owned()],
            recommendation: Some("Verify that the intended runtime has precedence.".to_owned()),
        };

        let json = serde_json::to_string(&finding).unwrap();
        let parsed: DiagnosticFinding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, finding);
        assert_ne!(parsed.finding, parsed.evidence.facts.join("\n"));
    }
}
