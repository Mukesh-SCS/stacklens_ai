use stacklens_core::{ContainerDetection, WslDetection};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OsRelease {
    pub id: Option<String>,
    pub id_like: Option<String>,
    pub pretty_name: Option<String>,
    pub version_id: Option<String>,
}

pub fn parse_os_release(text: &str) -> OsRelease {
    let mut out = OsRelease::default();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = unquote(value.trim());
        match key {
            "ID" => out.id = Some(value),
            "ID_LIKE" => out.id_like = Some(value),
            "PRETTY_NAME" => out.pretty_name = Some(value),
            "VERSION_ID" => out.version_id = Some(value),
            _ => {}
        }
    }
    out
}

fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        value[1..value.len() - 1].to_owned()
    } else {
        value.to_owned()
    }
}

pub fn detect_wsl(kernel_release: &str, proc_version: &str) -> WslDetection {
    let blob = format!("{kernel_release}\n{proc_version}").to_ascii_lowercase();
    if blob.contains("wsl2") {
        WslDetection::Wsl2
    } else if blob.contains("microsoft") || blob.contains("wsl") {
        WslDetection::Wsl1
    } else {
        WslDetection::NotDetected
    }
}

pub fn detect_container(dockerenv_exists: bool, cgroup: &str) -> ContainerDetection {
    if dockerenv_exists {
        return ContainerDetection::Docker;
    }
    let lower = cgroup.to_ascii_lowercase();
    if lower.contains("/docker") || (lower.contains("docker") && lower.contains(".scope")) {
        return ContainerDetection::Docker;
    }
    ContainerDetection::NotDetected
}

#[cfg(test)]
mod tests {
    use super::{detect_container, detect_wsl, parse_os_release};
    use stacklens_core::{ContainerDetection, WslDetection};

    #[test]
    fn os_release_ubuntu() {
        let parsed = parse_os_release(
            r#"
PRETTY_NAME="Ubuntu 24.04.1 LTS"
NAME="Ubuntu"
VERSION_ID="24.04"
ID=ubuntu
ID_LIKE=debian
"#,
        );
        assert_eq!(parsed.id.as_deref(), Some("ubuntu"));
        assert_eq!(parsed.id_like.as_deref(), Some("debian"));
        assert_eq!(parsed.pretty_name.as_deref(), Some("Ubuntu 24.04.1 LTS"));
        assert_eq!(parsed.version_id.as_deref(), Some("24.04"));
    }

    #[test]
    fn wsl2_from_kernel() {
        assert_eq!(
            detect_wsl("5.15.153.1-microsoft-standard-WSL2", ""),
            WslDetection::Wsl2
        );
    }

    #[test]
    fn wsl1_from_microsoft_kernel() {
        assert_eq!(detect_wsl("4.4.0-19041-Microsoft", ""), WslDetection::Wsl1);
    }

    #[test]
    fn native_linux_is_not_wsl() {
        assert_eq!(
            detect_wsl("6.8.0-40-generic", "Linux version 6.8.0-40-generic"),
            WslDetection::NotDetected
        );
    }

    #[test]
    fn dockerenv_file() {
        assert_eq!(detect_container(true, ""), ContainerDetection::Docker);
    }

    #[test]
    fn docker_cgroup() {
        assert_eq!(
            detect_container(false, "0::/system.slice/docker-abc.scope"),
            ContainerDetection::Docker
        );
    }
}
