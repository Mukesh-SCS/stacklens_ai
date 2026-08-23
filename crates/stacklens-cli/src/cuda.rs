use std::io::Write;

use stacklens_core::{
    CudaInfo, CudaRuntimeLibrary, CudaToolkit, DriverApiCapability, ForwardCompatPackage, NvccInfo,
    PathSelector, Probe, ToolkitVersionSource,
};

use crate::capture::current_snapshot;

pub fn run(stdout: &mut dyn Write) -> anyhow::Result<()> {
    let snapshot = current_snapshot();
    writeln!(stdout, "CUDA")?;
    writeln!(stdout)?;
    match snapshot.cuda.as_collected() {
        Some(info) => write_cuda(stdout, info)?,
        None => writeln!(stdout, "CUDA inspection did not produce a collection.")?,
    }
    for issue in snapshot.cuda.issues() {
        writeln!(stdout, "issue: {} ({})", issue.detail, issue.source)?;
    }
    Ok(())
}

fn write_cuda(stdout: &mut dyn Write, info: &CudaInfo) -> anyhow::Result<()> {
    writeln!(stdout, "Driver API capability")?;
    write_driver_api(stdout, info.driver_api_capability.as_ref())?;
    writeln!(stdout)?;

    writeln!(stdout, "Toolkits")?;
    if info.toolkits.is_empty() {
        writeln!(stdout, "  none detected")?;
    } else {
        for toolkit in &info.toolkits {
            write_toolkit(stdout, toolkit)?;
        }
    }
    writeln!(stdout)?;

    writeln!(stdout, "Selectors")?;
    write_selectors(stdout, info)?;
    writeln!(stdout)?;

    if let Some(nvcc) = &info.nvcc {
        writeln!(stdout, "PATH nvcc")?;
        write_nvcc(stdout, nvcc)?;
        writeln!(stdout)?;
    }

    writeln!(stdout, "Runtime library candidates")?;
    if info.runtime_libraries.is_empty() {
        writeln!(stdout, "  none detected")?;
    } else {
        for library in &info.runtime_libraries {
            write_runtime(stdout, library)?;
        }
    }
    writeln!(stdout)?;

    writeln!(stdout, "Forward compatibility")?;
    write_forward_compat(stdout, &info.forward_compat)?;

    if !info.compatibility.is_empty() {
        writeln!(stdout)?;
        writeln!(stdout, "Compatibility foundation")?;
        for item in &info.compatibility {
            writeln!(stdout, "  CUDA {}.x: {}", item.cuda_major, item.note)?;
        }
    }

    if let Some(context) = &info.container_context {
        writeln!(stdout)?;
        writeln!(stdout, "Container")?;
        writeln!(stdout, "  {context}")?;
    }

    Ok(())
}

fn write_driver_api(
    stdout: &mut dyn Write,
    capability: Option<&DriverApiCapability>,
) -> anyhow::Result<()> {
    match capability {
        Some(capability) => {
            writeln!(stdout, "  {}", capability.raw)?;
            writeln!(stdout, "  source: {}", capability.source)?;
        }
        None => writeln!(stdout, "  (not available)")?,
    }
    Ok(())
}

fn write_toolkit(stdout: &mut dyn Write, toolkit: &CudaToolkit) -> anyhow::Result<()> {
    writeln!(stdout)?;
    writeln!(stdout, "  {}", toolkit.path)?;
    match &toolkit.version {
        Some(version) => writeln!(stdout, "    version: {}", version.raw)?,
        None => writeln!(stdout, "    version: (not determined)")?,
    }
    if let Some(source) = toolkit.version_source {
        writeln!(
            stdout,
            "    version source: {}",
            format_version_source(source)
        )?;
    }
    if toolkit.is_symlink {
        writeln!(
            stdout,
            "    resolved: {}",
            toolkit.resolved_path.as_deref().unwrap_or("(unresolved)")
        )?;
    }
    writeln!(
        stdout,
        "    nvcc: {}",
        if toolkit.nvcc_present {
            "present"
        } else {
            "not present"
        }
    )?;
    writeln!(
        stdout,
        "    libcudart: {}",
        if toolkit.libcudart_present {
            "present"
        } else {
            "not present"
        }
    )?;
    Ok(())
}

fn write_selectors(stdout: &mut dyn Write, info: &CudaInfo) -> anyhow::Result<()> {
    if info.selectors.convenience_paths.is_empty()
        && info.selectors.cuda_home.is_none()
        && info.selectors.cuda_path.is_none()
        && info.selectors.cuda_root.is_none()
        && info.selectors.path_nvcc.is_none()
    {
        writeln!(stdout, "  none detected")?;
        return Ok(());
    }
    for selector in &info.selectors.convenience_paths {
        write_path_selector(stdout, selector)?;
    }
    write_named_selector(stdout, "CUDA_HOME", info.selectors.cuda_home.as_ref())?;
    write_named_selector(stdout, "CUDA_PATH", info.selectors.cuda_path.as_ref())?;
    write_named_selector(stdout, "CUDA_ROOT", info.selectors.cuda_root.as_ref())?;
    match &info.selectors.path_nvcc {
        Some(path) => {
            writeln!(stdout)?;
            writeln!(stdout, "  PATH nvcc:")?;
            writeln!(stdout, "    {path}")?;
        }
        None => {
            writeln!(stdout)?;
            writeln!(stdout, "  PATH nvcc:")?;
            writeln!(stdout, "    (not found)")?;
        }
    }
    Ok(())
}

fn write_named_selector(
    stdout: &mut dyn Write,
    name: &str,
    selector: Option<&PathSelector>,
) -> anyhow::Result<()> {
    writeln!(stdout)?;
    writeln!(stdout, "  {name}:")?;
    match selector {
        Some(selector) => write_path_selector_body(stdout, selector)?,
        None => writeln!(stdout, "    (not set)")?,
    }
    Ok(())
}

fn write_path_selector(stdout: &mut dyn Write, selector: &PathSelector) -> anyhow::Result<()> {
    writeln!(stdout)?;
    writeln!(stdout, "  {}:", selector.path)?;
    write_path_selector_body(stdout, selector)
}

fn write_path_selector_body(stdout: &mut dyn Write, selector: &PathSelector) -> anyhow::Result<()> {
    if let Some(resolved) = &selector.resolved_path {
        writeln!(stdout, "    -> {resolved}")?;
    } else if let Some(target) = &selector.link_target {
        writeln!(stdout, "    -> {target}")?;
    } else {
        writeln!(stdout, "    {}", selector.path)?;
    }
    if let Some(issue) = &selector.issue {
        writeln!(stdout, "    issue: {issue}")?;
    }
    Ok(())
}

fn write_nvcc(stdout: &mut dyn Write, nvcc: &NvccInfo) -> anyhow::Result<()> {
    writeln!(stdout, "  {}", nvcc.path)?;
    match &nvcc.compilation_tools_version {
        Some(version) => writeln!(stdout, "  compilation tools: {}", version.raw)?,
        None => writeln!(stdout, "  compilation tools: (not parsed)")?,
    }
    if let Some(release) = &nvcc.raw_release {
        writeln!(stdout, "  {release}")?;
    }
    if let Some(toolkit) = &nvcc.associated_toolkit {
        writeln!(stdout, "  associated toolkit: {toolkit}")?;
    }
    Ok(())
}

fn write_runtime(stdout: &mut dyn Write, library: &CudaRuntimeLibrary) -> anyhow::Result<()> {
    writeln!(stdout, "  {}", library.path)?;
    if let Some(target) = &library.symlink_target {
        writeln!(stdout, "    -> {target}")?;
    }
    writeln!(
        stdout,
        "    source: {}",
        match library.source {
            stacklens_core::RuntimeLibrarySource::Toolkit => "toolkit",
            stacklens_core::RuntimeLibrarySource::LdLibraryPath => "LD_LIBRARY_PATH",
        }
    )?;
    Ok(())
}

fn write_forward_compat(
    stdout: &mut dyn Write,
    packages: &[ForwardCompatPackage],
) -> anyhow::Result<()> {
    if packages.is_empty() {
        writeln!(stdout, "  none detected")?;
        return Ok(());
    }
    for package in packages {
        writeln!(stdout, "  {}", package.path)?;
        if package.libraries.is_empty() {
            writeln!(stdout, "    libraries: none listed")?;
        } else {
            for library in &package.libraries {
                writeln!(stdout, "    {library}")?;
            }
        }
        writeln!(
            stdout,
            "    referenced by LD_LIBRARY_PATH: {}",
            package.referenced_by_ld_library_path
        )?;
        writeln!(
            stdout,
            "    (presence is not proof that the loader will use these libraries)"
        )?;
    }
    Ok(())
}

fn format_version_source(source: ToolkitVersionSource) -> &'static str {
    match source {
        ToolkitVersionSource::ToolkitMetadata => "toolkit metadata",
        ToolkitVersionSource::VersionTxt => "version.txt",
        ToolkitVersionSource::Nvcc => "nvcc",
        ToolkitVersionSource::DirectoryName => "directory name",
    }
}

pub fn write_doctor_summary(stdout: &mut dyn Write, cuda: &Probe<CudaInfo>) -> anyhow::Result<()> {
    writeln!(stdout, "CUDA")?;
    match cuda.as_collected() {
        Some(info) => {
            writeln!(stdout, "  toolkit installations: {}", info.toolkits.len())?;
            writeln!(
                stdout,
                "  CUDA_HOME: {}",
                info.environment.cuda_home.as_deref().unwrap_or("(not set)")
            )?;
            match info
                .nvcc
                .as_ref()
                .and_then(|nvcc| nvcc.compilation_tools_version.as_ref())
            {
                Some(version) => writeln!(stdout, "  PATH nvcc: {}", version.raw)?,
                None => match info.selectors.path_nvcc.as_deref() {
                    Some(path) => writeln!(stdout, "  PATH nvcc: {path}")?,
                    None => writeln!(stdout, "  PATH nvcc: (not found)")?,
                },
            }
            match info.driver_api_capability.as_ref() {
                Some(capability) => {
                    writeln!(stdout, "  driver API capability: {}", capability.raw)?;
                }
                None => writeln!(stdout, "  driver API capability: (not available)")?,
            }
        }
        None => writeln!(stdout, "  not collected")?,
    }
    for issue in cuda.issues() {
        writeln!(stdout, "  issue: {} ({})", issue.detail, issue.source)?;
    }
    Ok(())
}
