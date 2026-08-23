use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};

use stacklens_core::EnvironmentInfo;

/// Diagnostic environment variables that are collected.
///
/// This is an allowlist. Tokens, cloud credentials, and other secrets are
/// omitted even if they are present in the process environment.
/// Values may still contain usernames and local paths.
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
    collect_environment(
        DIAGNOSTIC_ENV_VARS
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| (*name, value))),
    )
}

pub fn collect_environment<I, K, V>(vars: I) -> EnvironmentInfo
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let incoming: Vec<(OsString, OsString)> = vars
        .into_iter()
        .map(|(key, value)| (key.as_ref().to_owned(), value.as_ref().to_owned()))
        .collect();

    let mut variables = BTreeMap::new();
    for allowed in DIAGNOSTIC_ENV_VARS {
        if let Some((_, value)) = incoming
            .iter()
            .find(|(key, _)| env_name_eq(key.as_os_str(), OsStr::new(allowed)))
        {
            variables.insert((*allowed).to_owned(), value.to_string_lossy().into_owned());
        }
    }

    EnvironmentInfo { variables }
}

fn env_name_eq(observed: &OsStr, allowed: &OsStr) -> bool {
    if cfg!(windows) {
        observed
            .to_string_lossy()
            .eq_ignore_ascii_case(&allowed.to_string_lossy())
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

    #[cfg(unix)]
    #[test]
    fn non_unicode_value_is_lossy_not_a_panic() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let value = OsString::from_vec(vec![0xff, 0xfe, b'/', b'c', b'u', b'd', b'a']);
        let info = collect_environment([(OsString::from("CUDA_HOME"), value)]);
        let stored = info.variables.get("CUDA_HOME").expect("CUDA_HOME");
        assert!(stored.contains("/cuda"));
        assert!(stored.contains('\u{FFFD}'));
    }
}
