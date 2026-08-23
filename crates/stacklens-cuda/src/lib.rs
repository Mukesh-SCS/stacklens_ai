mod compat;
mod discover;
mod nvcc;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use stacklens_core::{
    ContainerDetection, CudaEnvironment, CudaInfo, CudaSelectors, DriverApiCapability,
    EnvironmentInfo, Probe, ProbeIssue, RuntimeLibrarySource, SystemInfo, ToolkitVersionSource,
};

use crate::discover::{
    associated_toolkit, candidate_paths_from_roots, collect_forward_compat, dedup_toolkits,
    inspect_toolkit, libcudart_in_toolkit, list_named_libs, observe_path,
    path_exists_or_dangling_symlink, runtime_from_paths, split_search_path, to_selector,
};
use crate::nvcc::{find_nvcc_on_path, nvcc_info, parse_nvcc_version, run_nvcc_version};

pub struct InspectOptions<'a> {
    pub environment: &'a EnvironmentInfo,
    pub search_roots: Vec<PathBuf>,
    pub extra_candidate_paths: Vec<PathBuf>,
    pub driver_api_capability: Option<DriverApiCapability>,
    pub driver_version: Option<String>,
    pub container: ContainerDetection,
    pub execute_nvcc: bool,
    pub nvcc_stdout: BTreeMap<String, String>,
}

pub fn default_search_roots() -> Vec<PathBuf> {
    if cfg!(target_os = "linux") {
        vec![PathBuf::from("/usr/local"), PathBuf::from("/opt")]
    } else {
        Vec::new()
    }
}

pub fn inspect_live(
    environment: &EnvironmentInfo,
    system: &SystemInfo,
    driver_api_capability: Option<DriverApiCapability>,
    driver_version: Option<String>,
) -> Probe<CudaInfo> {
    inspect(InspectOptions {
        environment,
        search_roots: default_search_roots(),
        extra_candidate_paths: Vec::new(),
        driver_api_capability,
        driver_version,
        container: system.container.clone(),
        execute_nvcc: true,
        nvcc_stdout: BTreeMap::new(),
    })
}

pub fn inspect(options: InspectOptions<'_>) -> Probe<CudaInfo> {
    let mut issues = Vec::new();
    let env_vars = &options.environment.variables;
    let path_entries = env_vars
        .get("PATH")
        .map(|value| split_search_path(value))
        .unwrap_or_default();
    let ld_library_path_entries = env_vars
        .get("LD_LIBRARY_PATH")
        .map(|value| split_search_path(value))
        .unwrap_or_default();

    let environment = CudaEnvironment {
        cuda_home: env_vars.get("CUDA_HOME").cloned(),
        cuda_path: env_vars.get("CUDA_PATH").cloned(),
        cuda_root: env_vars.get("CUDA_ROOT").cloned(),
        path_entries: path_entries.clone(),
        ld_library_path_entries: ld_library_path_entries.clone(),
    };

    let mut candidates = candidate_paths_from_roots(&options.search_roots);
    for key in ["CUDA_HOME", "CUDA_PATH", "CUDA_ROOT"] {
        if let Some(value) = env_vars.get(key) {
            candidates.push(PathBuf::from(value));
        }
    }
    candidates.extend(options.extra_candidate_paths.iter().cloned());

    let mut toolkits = Vec::new();
    let mut seen = BTreeSet::new();
    for path in candidates {
        let key = path.display().to_string();
        if !seen.insert(key) {
            continue;
        }
        if let Some(toolkit) = inspect_toolkit(&path) {
            toolkits.push(toolkit);
        }
    }
    let mut toolkits = dedup_toolkits(toolkits);

    let mut selectors = CudaSelectors::default();
    for root in &options.search_roots {
        let convenience = root.join("cuda");
        if path_exists_or_dangling_symlink(&convenience) {
            selectors
                .convenience_paths
                .push(to_selector(&observe_path(&convenience)));
        }
    }
    selectors
        .convenience_paths
        .sort_by(|left, right| left.path.cmp(&right.path));

    selectors.cuda_home = env_selector(env_vars.get("CUDA_HOME"));
    selectors.cuda_path = env_selector(env_vars.get("CUDA_PATH"));
    selectors.cuda_root = env_selector(env_vars.get("CUDA_ROOT"));

    let path_nvcc = find_nvcc_on_path(&path_entries);
    selectors.path_nvcc = path_nvcc.as_ref().map(|path| path.display().to_string());

    let nvcc = path_nvcc.as_ref().map(|path| {
        let stdout = match options.nvcc_stdout.get(&path.display().to_string()) {
            Some(text) => text.clone(),
            None if options.execute_nvcc => match run_nvcc_version(path) {
                Ok(text) => text,
                Err(issue) => {
                    issues.push(issue);
                    return nvcc_info(
                        path.clone(),
                        parse_nvcc_version(""),
                        associated_toolkit(path, &toolkits),
                    );
                }
            },
            None => {
                return nvcc_info(
                    path.clone(),
                    parse_nvcc_version(""),
                    associated_toolkit(path, &toolkits),
                );
            }
        };
        let parsed = parse_nvcc_version(&stdout);
        if parsed.compilation_tools_version.is_none() && !stdout.trim().is_empty() {
            issues.push(ProbeIssue {
                source: path.display().to_string(),
                kind: stacklens_core::ProbeIssueKind::Malformed,
                detail: "nvcc --version did not contain a compilation-tools release line"
                    .to_owned(),
            });
        }
        nvcc_info(path.clone(), parsed, associated_toolkit(path, &toolkits))
    });

    if let Some(nvcc) = &nvcc {
        if let (Some(version), Some(toolkit_path)) =
            (&nvcc.compilation_tools_version, &nvcc.associated_toolkit)
        {
            if let Some(toolkit) = toolkits
                .iter_mut()
                .find(|toolkit| &toolkit.path == toolkit_path)
            {
                if toolkit.version.is_none() {
                    toolkit.version = Some(version.clone());
                    toolkit.version_source = Some(ToolkitVersionSource::Nvcc);
                }
            }
        }
    }

    let mut runtime_libraries = Vec::new();
    let mut seen_runtime = BTreeSet::new();
    for toolkit in &toolkits {
        let root = toolkit
            .resolved_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&toolkit.path));
        for library in runtime_from_paths(
            &libcudart_in_toolkit(&root),
            RuntimeLibrarySource::Toolkit,
            Some(Path::new(&toolkit.path)),
        ) {
            if seen_runtime.insert(library.path.clone()) {
                runtime_libraries.push(library);
            }
        }
    }
    for entry in &ld_library_path_entries {
        let dir = Path::new(entry);
        for library in runtime_from_paths(
            &list_named_libs(dir, discover::is_libcudart_name),
            RuntimeLibrarySource::LdLibraryPath,
            Some(dir),
        ) {
            if seen_runtime.insert(library.path.clone()) {
                runtime_libraries.push(library);
            }
        }
    }
    runtime_libraries.sort_by(|left, right| left.path.cmp(&right.path));

    let toolkit_roots: Vec<PathBuf> = toolkits
        .iter()
        .map(|toolkit| {
            toolkit
                .resolved_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(&toolkit.path))
        })
        .collect();
    let extra_compat: Vec<PathBuf> = options
        .search_roots
        .iter()
        .filter_map(|root| {
            let entries = std::fs::read_dir(root).ok()?;
            Some(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.starts_with("cuda-compat-"))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect();
    let forward_compat =
        collect_forward_compat(&toolkit_roots, &extra_compat, &ld_library_path_entries);

    let majors: Vec<u16> = toolkits
        .iter()
        .filter_map(|toolkit| toolkit.version.as_ref().map(|version| version.major))
        .collect();
    let compatibility = compat::evidence_for_majors(&majors, options.driver_version.as_deref());

    let container_context = match options.container {
        ContainerDetection::Docker => Some(
            "container detected; inspection is limited to the current mount namespace. Host CUDA was not inspected.".to_owned(),
        ),
        _ => None,
    };

    Probe::collected_with_issues(
        CudaInfo {
            toolkits,
            selectors,
            nvcc,
            runtime_libraries,
            driver_api_capability: options.driver_api_capability,
            forward_compat,
            environment,
            compatibility,
            container_context,
        },
        issues,
    )
}

fn env_selector(value: Option<&String>) -> Option<stacklens_core::PathSelector> {
    value.map(|path| to_selector(&observe_path(Path::new(path))))
}

#[cfg(test)]
mod tests {
    use super::{inspect, InspectOptions};
    use stacklens_core::{
        ContainerDetection, CudaVersion, DriverApiCapability, EnvironmentInfo, ToolkitVersionSource,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join("stacklens-cuda-tests")
                .join(format!(
                    "{}-{}-{}",
                    label,
                    std::process::id(),
                    NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
                ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("temp root");
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn write_toolkit(root: &Path, name: &str, version: &str, metadata: bool) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("bin")).expect("bin");
        fs::create_dir_all(dir.join("lib64")).expect("lib64");
        fs::write(dir.join("bin").join("nvcc"), []).expect("nvcc");
        let major = version.split('.').next().expect("major");
        fs::write(dir.join("lib64").join(format!("libcudart.so.{major}")), []).expect("libcudart");
        if metadata {
            fs::write(
                dir.join("version.json"),
                format!(r#"{{"cuda":{{"name":"CUDA SDK","version":"{version}"}}}}"#),
            )
            .expect("version.json");
        }
        dir
    }

    fn env(pairs: &[(&str, &str)]) -> EnvironmentInfo {
        let mut variables = BTreeMap::new();
        for (key, value) in pairs {
            variables.insert((*key).to_owned(), (*value).to_owned());
        }
        EnvironmentInfo { variables }
    }

    fn join_paths(parts: &[&str]) -> String {
        let sep = if cfg!(windows) { ";" } else { ":" };
        parts.join(sep)
    }

    fn inspect_roots(root: &Path, environment: &EnvironmentInfo) -> stacklens_core::CudaInfo {
        inspect(InspectOptions {
            environment,
            search_roots: vec![root.to_owned()],
            extra_candidate_paths: Vec::new(),
            driver_api_capability: None,
            driver_version: None,
            container: ContainerDetection::NotDetected,
            execute_nvcc: false,
            nvcc_stdout: BTreeMap::new(),
        })
        .as_collected()
        .expect("cuda collected")
        .clone()
    }

    #[cfg(unix)]
    fn try_dir_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[test]
    fn ignores_cuda_named_directory_without_artifacts() {
        let tmp = TempRoot::new("named-only");
        fs::create_dir_all(tmp.path.join("cuda-99.0")).unwrap();
        let info = inspect_roots(&tmp.path, &env(&[]));
        assert!(info.toolkits.is_empty());
    }

    #[test]
    fn multiple_toolkits_have_stable_order() {
        let tmp = TempRoot::new("multi");
        write_toolkit(&tmp.path, "cuda-13.0", "13.0.0", true);
        write_toolkit(&tmp.path, "cuda-11.8", "11.8.0", true);
        write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let info = inspect_roots(&tmp.path, &env(&[]));
        let paths: Vec<_> = info.toolkits.iter().map(|t| t.path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
        assert_eq!(info.toolkits.len(), 3);
        assert_eq!(info.toolkits[0].version.as_ref().map(|v| v.major), Some(11));
    }

    #[test]
    fn metadata_beats_directory_name() {
        let tmp = TempRoot::new("meta");
        write_toolkit(&tmp.path, "cuda-12.8", "12.4.1", true);
        let info = inspect_roots(&tmp.path, &env(&[]));
        let toolkit = &info.toolkits[0];
        assert_eq!(
            toolkit
                .version
                .as_ref()
                .map(|v| (v.major, v.minor, v.patch)),
            Some((12, 4, Some(1)))
        );
        assert_eq!(
            toolkit.version_source,
            Some(ToolkitVersionSource::ToolkitMetadata)
        );
    }

    #[test]
    fn directory_name_is_lower_confidence_source() {
        let tmp = TempRoot::new("dirname");
        write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", false);
        let info = inspect_roots(&tmp.path, &env(&[]));
        assert_eq!(
            info.toolkits[0].version_source,
            Some(ToolkitVersionSource::DirectoryName)
        );
        assert_eq!(
            info.toolkits[0]
                .version
                .as_ref()
                .map(|v| (v.major, v.minor)),
            Some((12, 8))
        );
    }

    #[test]
    fn cuda_home_and_path_nvcc_disagreement_is_preserved() {
        let tmp = TempRoot::new("disagree");
        let t128 = write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let t126 = write_toolkit(&tmp.path, "cuda-12.6", "12.6.0", true);
        write_toolkit(&tmp.path, "cuda-13.0", "13.0.0", true);
        let home = t128.display().to_string();
        let path = join_paths(&[&t126.join("bin").display().to_string(), "/usr/bin"]);
        let info = inspect_roots(
            &tmp.path,
            &env(&[("CUDA_HOME", home.as_str()), ("PATH", path.as_str())]),
        );
        assert_eq!(info.toolkits.len(), 3);
        assert_eq!(
            info.selectors.cuda_home.as_ref().map(|s| s.path.as_str()),
            Some(home.as_str())
        );
        assert_eq!(
            info.selectors.path_nvcc.as_deref(),
            Some(t126.join("bin").join("nvcc").display().to_string().as_str())
        );
        assert_ne!(
            info.selectors.cuda_home.as_ref().map(|s| s.path.as_str()),
            info.selectors.path_nvcc.as_deref()
        );
    }

    #[test]
    fn path_nvcc_uses_first_path_entry() {
        let tmp = TempRoot::new("path-order");
        let first = write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let second = write_toolkit(&tmp.path, "cuda-13.0", "13.0.0", true);
        let path = join_paths(&[
            &first.join("bin").display().to_string(),
            &second.join("bin").display().to_string(),
        ]);
        let info = inspect_roots(&tmp.path, &env(&[("PATH", path.as_str())]));
        assert!(info
            .selectors
            .path_nvcc
            .as_deref()
            .unwrap()
            .contains("cuda-12.8"));
        assert!(!info
            .selectors
            .path_nvcc
            .as_deref()
            .unwrap()
            .contains("cuda-13.0"));
    }

    #[test]
    fn nvcc_missing_on_path() {
        let tmp = TempRoot::new("nvcc-missing");
        write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let info = inspect_roots(
            &tmp.path,
            &env(&[(
                "PATH",
                tmp.path.join("empty-bin").display().to_string().as_str(),
            )]),
        );
        assert!(info.selectors.path_nvcc.is_none());
        assert!(info.nvcc.is_none());
    }

    #[test]
    fn nvcc_version_override_and_association() {
        let tmp = TempRoot::new("nvcc-run");
        let toolkit = write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let nvcc = toolkit.join("bin").join("nvcc");
        let path = join_paths(&[&toolkit.join("bin").display().to_string()]);
        let mut stdout = BTreeMap::new();
        stdout.insert(
            nvcc.display().to_string(),
            fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/cuda/nvcc-12.8.txt"),
            )
            .unwrap(),
        );
        let environment = env(&[("PATH", path.as_str())]);
        let info = inspect(InspectOptions {
            environment: &environment,
            search_roots: vec![tmp.path.clone()],
            extra_candidate_paths: Vec::new(),
            driver_api_capability: None,
            driver_version: None,
            container: ContainerDetection::NotDetected,
            execute_nvcc: false,
            nvcc_stdout: stdout,
        })
        .as_collected()
        .unwrap()
        .clone();
        let nvcc_info = info.nvcc.expect("nvcc");
        assert_eq!(
            nvcc_info
                .compilation_tools_version
                .as_ref()
                .map(|v| (v.major, v.minor)),
            Some((12, 8))
        );
        assert_eq!(
            nvcc_info.associated_toolkit.as_deref(),
            Some(toolkit.display().to_string().as_str())
        );
    }

    #[test]
    fn runtime_libraries_from_toolkit_and_ld_library_path() {
        let tmp = TempRoot::new("runtime");
        let toolkit = write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        fs::write(toolkit.join("lib64").join("libcudart.so"), []).unwrap();
        let extra = tmp.path.join("venv-lib");
        fs::create_dir_all(&extra).unwrap();
        fs::write(extra.join("libcudart.so.12"), []).unwrap();
        let ld = extra.display().to_string();
        let info = inspect_roots(&tmp.path, &env(&[("LD_LIBRARY_PATH", ld.as_str())]));
        assert!(info.runtime_libraries.len() >= 2);
        assert!(info
            .runtime_libraries
            .iter()
            .any(|lib| lib.path.contains("venv-lib")));
        let paths: Vec<_> = info
            .runtime_libraries
            .iter()
            .map(|lib| lib.path.as_str())
            .collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }

    #[test]
    fn forward_compat_directory_is_detected_not_activated() {
        let tmp = TempRoot::new("compat");
        let toolkit = write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let compat = toolkit.join("compat");
        fs::create_dir_all(&compat).unwrap();
        fs::write(compat.join("libcuda.so.1"), []).unwrap();
        fs::write(compat.join("libnvidia-ptxjitcompiler.so.1"), []).unwrap();
        let info = inspect_roots(&tmp.path, &env(&[]));
        assert_eq!(info.forward_compat.len(), 1);
        assert!(!info.forward_compat[0].libraries.is_empty());
        assert!(!info.forward_compat[0].referenced_by_ld_library_path);
    }

    #[test]
    fn driver_api_capability_stays_separate() {
        let tmp = TempRoot::new("driver-api");
        write_toolkit(&tmp.path, "cuda-12.8", "12.8.0", true);
        let environment = env(&[]);
        let info = inspect(InspectOptions {
            environment: &environment,
            search_roots: vec![tmp.path.clone()],
            extra_candidate_paths: Vec::new(),
            driver_api_capability: Some(DriverApiCapability {
                raw: "13.0".to_owned(),
                version: CudaVersion::parse("13.0"),
                source: "nvidia-smi".to_owned(),
            }),
            driver_version: Some("580.82.07".to_owned()),
            container: ContainerDetection::NotDetected,
            execute_nvcc: false,
            nvcc_stdout: BTreeMap::new(),
        })
        .as_collected()
        .unwrap()
        .clone();
        assert_eq!(
            info.driver_api_capability.as_ref().map(|c| c.raw.as_str()),
            Some("13.0")
        );
        assert_eq!(
            info.toolkits[0]
                .version
                .as_ref()
                .map(|v| (v.major, v.minor)),
            Some((12, 8))
        );
        assert_ne!(
            info.driver_api_capability
                .as_ref()
                .and_then(|c| c.version.as_ref())
                .map(|v| (v.major, v.minor)),
            info.toolkits[0]
                .version
                .as_ref()
                .map(|v| (v.major, v.minor))
        );
        assert_eq!(info.compatibility[0].cuda_major, 12);
        assert_eq!(
            info.compatibility[0].driver_branch_meets_documented_minimum,
            Some(true)
        );
        assert!(info.compatibility[0]
            .note
            .contains("not a full compatibility determination"));
    }

    #[test]
    fn ld_library_path_order_is_preserved() {
        let tmp = TempRoot::new("ld-order");
        let first = tmp.path.join("a");
        let second = tmp.path.join("b");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let value = join_paths(&[
            first.display().to_string().as_str(),
            second.display().to_string().as_str(),
        ]);
        let info = inspect_roots(&tmp.path, &env(&[("LD_LIBRARY_PATH", value.as_str())]));
        assert_eq!(
            info.environment.ld_library_path_entries,
            vec![first.display().to_string(), second.display().to_string()]
        );
    }

    #[cfg(unix)]
    #[test]
    fn usr_local_cuda_symlink_resolution() {
        let tmp = TempRoot::new("symlink");
        let target = write_toolkit(&tmp.path, "cuda-13.0", "13.0.0", true);
        let link = tmp.path.join("cuda");
        assert!(try_dir_symlink(&target, &link));
        let info = inspect_roots(&tmp.path, &env(&[]));
        assert_eq!(info.toolkits.len(), 1);
        assert!(info.toolkits[0].path.contains("cuda-13.0"));
        let selector = &info.selectors.convenience_paths[0];
        assert!(selector.is_symlink);
        assert!(selector
            .resolved_path
            .as_deref()
            .unwrap()
            .contains("cuda-13.0"));
        assert_eq!(selector.path, link.display().to_string());
    }

    #[cfg(unix)]
    #[test]
    fn broken_symlink_does_not_crash() {
        let tmp = TempRoot::new("broken");
        let link = tmp.path.join("cuda");
        assert!(try_dir_symlink(&tmp.path.join("cuda-missing"), &link));
        let info = inspect_roots(&tmp.path, &env(&[]));
        assert!(info.toolkits.is_empty());
        assert_eq!(info.selectors.convenience_paths.len(), 1);
        assert!(info.selectors.convenience_paths[0].is_symlink);
        assert!(info.selectors.convenience_paths[0].issue.is_some());
    }

    #[test]
    fn inspect_does_not_require_local_cuda() {
        let environment = env(&[]);
        let info = inspect(InspectOptions {
            environment: &environment,
            search_roots: vec![std::env::temp_dir().join("stacklens-cuda-no-such-root")],
            extra_candidate_paths: Vec::new(),
            driver_api_capability: None,
            driver_version: None,
            container: ContainerDetection::NotDetected,
            execute_nvcc: false,
            nvcc_stdout: BTreeMap::new(),
        });
        let data = info.as_collected().expect("collected");
        assert!(data.toolkits.is_empty());
        assert!(data.nvcc.is_none());
    }
}
