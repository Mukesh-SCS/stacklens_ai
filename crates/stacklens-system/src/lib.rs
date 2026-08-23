//! Host collectors for StackLens.

mod env;
mod linux;
mod parse;

use stacklens_core::model::SystemInfo;
use stacklens_core::Endian;

pub use env::{collect_environment, collect_environment_from_os, DIAGNOSTIC_ENV_VARS};
pub use parse::{detect_container, detect_wsl, parse_os_release, OsRelease};

pub fn collect_system() -> SystemInfo {
    let mut info = SystemInfo {
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        family: std::env::consts::FAMILY.to_owned(),
        pointer_width: (std::mem::size_of::<usize>() * 8) as u32,
        endian: if cfg!(target_endian = "little") {
            Endian::Little
        } else {
            Endian::Big
        },
        os_id: None,
        os_id_like: None,
        os_pretty_name: None,
        os_version_id: None,
        kernel_release: None,
        container: stacklens_core::ContainerDetection::NotInspected,
        wsl: stacklens_core::WslDetection::NotInspected,
    };
    linux::enrich(&mut info);
    info
}

#[cfg(test)]
mod tests {
    use super::collect_system;

    #[test]
    fn system_matches_rustc_target() {
        let info = collect_system();
        assert_eq!(info.os, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
        assert_eq!(
            info.pointer_width,
            (std::mem::size_of::<usize>() * 8) as u32
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_kernel_release_is_read_when_available() {
        let info = collect_system();
        if std::path::Path::new("/proc/sys/kernel/osrelease").is_file() {
            assert!(info.kernel_release.is_some());
        }
    }
}
