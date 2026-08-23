use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use stacklens_core::{
    CudaRuntimeLibrary, CudaToolkit, CudaVersion, ForwardCompatPackage, PathSelector,
    RuntimeLibrarySource, ToolkitVersionSource,
};

const MAX_TOOLKITS: usize = 32;
const MAX_LIBS_PER_DIR: usize = 16;

#[derive(Debug, Clone)]
pub struct ObservedPath {
    pub path: PathBuf,
    pub is_symlink: bool,
    pub link_target: Option<PathBuf>,
    pub resolved_path: Option<PathBuf>,
    pub issue: Option<String>,
}

pub fn split_search_path(value: &str) -> Vec<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    value
        .split(sep)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn observe_path(path: &Path) -> ObservedPath {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            let is_symlink = meta.file_type().is_symlink();
            let link_target = if is_symlink {
                fs::read_link(path).ok()
            } else {
                None
            };
            let (resolved_path, issue) = if is_symlink {
                match fs::canonicalize(path) {
                    Ok(resolved) => (Some(resolved), None),
                    Err(err) => (None, Some(classify_path_error(&err))),
                }
            } else {
                (None, None)
            };
            ObservedPath {
                path: path.to_owned(),
                is_symlink,
                link_target,
                resolved_path,
                issue,
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => ObservedPath {
            path: path.to_owned(),
            is_symlink: false,
            link_target: None,
            resolved_path: None,
            issue: None,
        },
        Err(err) => ObservedPath {
            path: path.to_owned(),
            is_symlink: false,
            link_target: None,
            resolved_path: None,
            issue: Some(classify_path_error(&err)),
        },
    }
}

pub fn path_exists_or_dangling_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn classify_path_error(err: &std::io::Error) -> String {
    match err.kind() {
        ErrorKind::PermissionDenied => "permission denied".to_owned(),
        ErrorKind::NotFound => "broken symlink or missing target".to_owned(),
        _ => {
            let lower = err.to_string().to_ascii_lowercase();
            if lower.contains("loop") || lower.contains("cycle") {
                "symlink loop".to_owned()
            } else {
                err.to_string()
            }
        }
    }
}

pub fn to_selector(observed: &ObservedPath) -> PathSelector {
    PathSelector {
        path: observed.path.display().to_string(),
        is_symlink: observed.is_symlink,
        link_target: observed
            .link_target
            .as_ref()
            .map(|path| path.display().to_string()),
        resolved_path: observed
            .resolved_path
            .as_ref()
            .map(|path| path.display().to_string()),
        issue: observed.issue.clone(),
    }
}

pub fn candidate_paths_from_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for root in roots {
        let convenience = root.join("cuda");
        if path_exists_or_dangling_symlink(&convenience) {
            candidates.push(convenience);
        }
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with("cuda-") || name.starts_with("cuda-compat-") {
                candidates.push(entry.path());
            }
        }
    }
    candidates
}

pub fn inspect_toolkit(path: &Path) -> Option<CudaToolkit> {
    let observed = observe_path(path);
    let inspect_root = observed
        .resolved_path
        .clone()
        .unwrap_or_else(|| path.to_owned());
    if !inspect_root.is_dir() {
        return None;
    }

    let nvcc_present = crate::nvcc::is_nvcc_candidate(&inspect_root.join("bin").join("nvcc"));
    let libcudart = libcudart_in_toolkit(&inspect_root);
    let has_version_json = inspect_root.join("version.json").is_file();
    let has_version_txt = inspect_root.join("version.txt").is_file();
    if !nvcc_present && libcudart.is_empty() && !has_version_json && !has_version_txt {
        return None;
    }

    let (version, version_source) = resolve_toolkit_version(&inspect_root, path);

    Some(CudaToolkit {
        path: path.display().to_string(),
        is_symlink: observed.is_symlink,
        resolved_path: observed
            .resolved_path
            .as_ref()
            .map(|resolved| resolved.display().to_string()),
        version,
        version_source,
        nvcc_present,
        libcudart_present: !libcudart.is_empty(),
    })
}

fn resolve_toolkit_version(
    inspect_root: &Path,
    logical: &Path,
) -> (Option<CudaVersion>, Option<ToolkitVersionSource>) {
    if let Some(version) = read_version_json(&inspect_root.join("version.json")) {
        return (Some(version), Some(ToolkitVersionSource::ToolkitMetadata));
    }
    if let Some(version) = read_version_txt(&inspect_root.join("version.txt")) {
        return (Some(version), Some(ToolkitVersionSource::VersionTxt));
    }
    if let Some(version) =
        version_from_dirname(logical).or_else(|| version_from_dirname(inspect_root))
    {
        return (Some(version), Some(ToolkitVersionSource::DirectoryName));
    }
    (None, None)
}

fn read_version_json(path: &Path) -> Option<CudaVersion> {
    let text = fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = value
        .get("cuda")
        .and_then(|cuda| cuda.get("version"))
        .and_then(|version| version.as_str())?;
    CudaVersion::parse(raw)
}

fn read_version_txt(path: &Path) -> Option<CudaVersion> {
    let text = fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("CUDA Version ") {
            if let Some(version) = CudaVersion::parse(rest.trim()) {
                return Some(version);
            }
        }
        if let Some(version) = CudaVersion::parse(line) {
            return Some(version);
        }
    }
    None
}

pub fn version_from_dirname(path: &Path) -> Option<CudaVersion> {
    let name = path.file_name()?.to_str()?;
    let rest = name.strip_prefix("cuda-")?;
    CudaVersion::parse(rest)
}

pub fn toolkit_lib_dirs(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("lib64"),
        root.join("lib"),
        root.join("targets").join("x86_64-linux").join("lib"),
    ]
}

pub fn libcudart_in_toolkit(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for dir in toolkit_lib_dirs(root) {
        found.extend(list_named_libs(&dir, is_libcudart_name));
    }
    found
}

pub fn list_named_libs(dir: &Path, matches: fn(&str) -> bool) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        if found.len() >= MAX_LIBS_PER_DIR {
            break;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if matches(name) {
            found.push(entry.path());
        }
    }
    found.sort();
    found
}

pub fn is_libcudart_name(name: &str) -> bool {
    name == "libcudart.so" || name.starts_with("libcudart.so.")
}

pub fn is_compat_lib_name(name: &str) -> bool {
    name.starts_with("libcuda.so")
        || name.starts_with("libnvidia-ptxjitcompiler.so")
        || name.starts_with("libnvidia-nvvm.so")
}

pub fn runtime_from_paths(
    paths: &[PathBuf],
    source: RuntimeLibrarySource,
    source_path: Option<&Path>,
) -> Vec<CudaRuntimeLibrary> {
    let mut libraries = Vec::new();
    for path in paths {
        let symlink_target = fs::symlink_metadata(path)
            .ok()
            .filter(|meta| meta.file_type().is_symlink())
            .and_then(|_| fs::read_link(path).ok())
            .map(|target| target.display().to_string());
        libraries.push(CudaRuntimeLibrary {
            path: path.display().to_string(),
            symlink_target,
            source,
            source_path: source_path.map(|path| path.display().to_string()),
        });
    }
    libraries
}

pub fn collect_forward_compat(
    toolkit_roots: &[PathBuf],
    extra_dirs: &[PathBuf],
    ld_library_path: &[String],
) -> Vec<ForwardCompatPackage> {
    let mut dirs = Vec::new();
    for root in toolkit_roots {
        let compat = root.join("compat");
        if compat.is_dir() {
            dirs.push(compat);
        }
    }
    for dir in extra_dirs {
        if dir.is_dir() {
            dirs.push(dir.clone());
        }
    }
    dirs.sort();
    dirs.dedup();

    let mut packages = Vec::new();
    for dir in dirs {
        let libraries = list_named_libs(&dir, is_compat_lib_name);
        if libraries.is_empty() && !dir.ends_with("compat") {
            continue;
        }
        let referenced = ld_library_path
            .iter()
            .any(|entry| paths_refer_to_same(Path::new(entry), &dir));
        packages.push(ForwardCompatPackage {
            path: dir.display().to_string(),
            libraries: libraries
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
            referenced_by_ld_library_path: referenced,
        });
    }
    packages.sort_by(|left, right| left.path.cmp(&right.path));
    packages
}

fn paths_refer_to_same(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

pub fn dedup_toolkits(mut toolkits: Vec<CudaToolkit>) -> Vec<CudaToolkit> {
    toolkits.sort_by(|left, right| left.path.cmp(&right.path));
    let mut kept: Vec<CudaToolkit> = Vec::new();
    for toolkit in toolkits {
        let identity = toolkit
            .resolved_path
            .clone()
            .unwrap_or_else(|| toolkit.path.clone());
        if let Some(existing) = kept.iter_mut().find(|item| {
            let existing_id = item
                .resolved_path
                .clone()
                .unwrap_or_else(|| item.path.clone());
            existing_id == identity
        }) {
            if prefer_toolkit_identity(&toolkit, existing) {
                *existing = toolkit;
            }
            continue;
        }
        if kept.len() < MAX_TOOLKITS {
            kept.push(toolkit);
        }
    }
    kept.sort_by(|left, right| left.path.cmp(&right.path));
    kept
}

fn prefer_toolkit_identity(candidate: &CudaToolkit, existing: &CudaToolkit) -> bool {
    match (candidate.is_symlink, existing.is_symlink) {
        (false, true) => true,
        (true, false) => false,
        _ => {
            let candidate_named = version_from_dirname(Path::new(&candidate.path)).is_some();
            let existing_named = version_from_dirname(Path::new(&existing.path)).is_some();
            candidate_named && !existing_named
        }
    }
}

pub fn associated_toolkit(nvcc: &Path, toolkits: &[CudaToolkit]) -> Option<String> {
    let parent_bin = nvcc.parent()?;
    if parent_bin.file_name()?.to_str()? != "bin" {
        return None;
    }
    let toolkit_dir = parent_bin.parent()?;
    let logical = toolkit_dir.display().to_string();
    let resolved = fs::canonicalize(toolkit_dir).ok();
    toolkits.iter().find_map(|toolkit| {
        if toolkit.path == logical {
            return Some(toolkit.path.clone());
        }
        match (&toolkit.resolved_path, &resolved) {
            (Some(existing), Some(got)) if Path::new(existing) == got => Some(toolkit.path.clone()),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{is_libcudart_name, split_search_path, version_from_dirname};
    use std::path::Path;

    #[test]
    fn libcudart_name_filter() {
        assert!(is_libcudart_name("libcudart.so"));
        assert!(is_libcudart_name("libcudart.so.12"));
        assert!(!is_libcudart_name("libcudart_static.a"));
        assert!(!is_libcudart_name("libcuda.so.1"));
    }

    #[test]
    fn dirname_version() {
        let version = version_from_dirname(Path::new("/usr/local/cuda-12.8")).unwrap();
        assert_eq!((version.major, version.minor), (12, 8));
        assert!(version_from_dirname(Path::new("/usr/local/cuda")).is_none());
    }

    #[test]
    fn path_split_preserves_order() {
        let sep = if cfg!(windows) { ';' } else { ':' };
        let value = format!("/opt/first{sep}/opt/second{sep}/opt/first");
        let entries = split_search_path(&value);
        assert_eq!(
            entries,
            vec![
                "/opt/first".to_owned(),
                "/opt/second".to_owned(),
                "/opt/first".to_owned()
            ]
        );
    }
}
