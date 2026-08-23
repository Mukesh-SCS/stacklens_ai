use std::collections::BTreeMap;

use stacklens_core::EnvironmentInfo;

/// Diagnostic environment variables that are collected.
///
/// This is an allowlist. Tokens, cloud credentials, and other secrets are
/// omitted even if they are present in the process environment.
pub const DIAGNOSTIC_ENV_VARS: &[&str] = &[
    "CONDA_PREFIX",
    "CUDA_HOME",
    "CUDA_PATH",
    "CUDA_ROOT",
    "CUDA_VISIBLE_DEVICES",
    "CUDNN_PATH",
    "HIP_VISIBLE_DEVICES",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "LIBRARY_PATH",
    "NVIDIA_DRIVER_CAPABILITIES",
    "NVIDIA_VISIBLE_DEVICES",
    "PATH",
    "PYTHONPATH",
    "PYTORCH_CUDA_ALLOC_CONF",
    "ROCM_PATH",
    "TORCH_CUDA_ARCH_LIST",
    "TORCH_HOME",
    "VIRTUAL_ENV",
];

pub fn collect_environment_from_os() -> EnvironmentInfo {
    collect_environment(std::env::vars())
}

pub fn collect_environment<I, K, V>(vars: I) -> EnvironmentInfo
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: Into<String>,
{
    let incoming: Vec<(String, String)> = vars
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_owned(), value.into()))
        .collect();

    let mut variables = BTreeMap::new();
    for allowed in DIAGNOSTIC_ENV_VARS {
        if let Some((_, value)) = incoming.iter().find(|(key, _)| env_name_eq(key, allowed)) {
            variables.insert((*allowed).to_owned(), value.clone());
        }
    }

    EnvironmentInfo { variables }
}

fn env_name_eq(observed: &str, allowed: &str) -> bool {
    if cfg!(windows) {
        observed.eq_ignore_ascii_case(allowed)
    } else {
        observed == allowed
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_environment, DIAGNOSTIC_ENV_VARS};

    #[test]
    fn allowlist_keeps_cuda_home() {
        let info = collect_environment([
            ("CUDA_HOME", "/usr/local/cuda"),
            ("OPENAI_API_KEY", "sk-test"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
            ("GITHUB_TOKEN", "ghs_test"),
            ("HF_TOKEN", "hf_test"),
            ("DATABASE_URL", "postgres://example"),
        ]);
        assert_eq!(
            info.variables.get("CUDA_HOME").map(String::as_str),
            Some("/usr/local/cuda")
        );
        for secret in [
            "OPENAI_API_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "GITHUB_TOKEN",
            "HF_TOKEN",
            "DATABASE_URL",
        ] {
            assert!(!info.variables.contains_key(secret), "{secret} leaked");
        }
    }

    #[test]
    fn allowlist_is_sorted() {
        let mut sorted = DIAGNOSTIC_ENV_VARS.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, DIAGNOSTIC_ENV_VARS);
    }

    #[test]
    fn missing_allowlisted_vars_are_omitted() {
        let info = collect_environment::<_, &str, &str>([]);
        assert!(info.variables.is_empty());
    }
}
