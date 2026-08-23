use stacklens_core::model::SystemInfo;

#[cfg(not(target_os = "linux"))]
use stacklens_core::{ContainerDetection, WslDetection};

#[cfg(target_os = "linux")]
pub fn enrich(info: &mut SystemInfo) {
    use std::fs;
    use std::path::Path;

    use crate::parse::{detect_container, detect_wsl, parse_os_release};

    if let Ok(text) = fs::read_to_string("/etc/os-release") {
        let parsed = parse_os_release(&text);
        info.os_id = parsed.id;
        info.os_id_like = parsed.id_like;
        info.os_pretty_name = parsed.pretty_name;
        info.os_version_id = parsed.version_id;
    }

    let kernel_release = fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_owned());
    let proc_version = fs::read_to_string("/proc/version").unwrap_or_default();
    match kernel_release {
        Some(release) => {
            info.wsl = detect_wsl(&release, &proc_version);
            info.kernel_release = Some(release);
        }
        None => info.wsl = detect_wsl("", &proc_version),
    }

    let dockerenv = Path::new("/.dockerenv").exists();
    let cgroup = fs::read_to_string("/proc/1/cgroup").unwrap_or_default();
    info.container = detect_container(dockerenv, &cgroup);
}

#[cfg(not(target_os = "linux"))]
pub fn enrich(info: &mut SystemInfo) {
    info.container = ContainerDetection::NotInspected;
    info.wsl = WslDetection::NotInspected;
}
