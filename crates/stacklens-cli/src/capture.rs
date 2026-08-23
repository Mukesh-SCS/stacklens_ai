use stacklens_core::{
    unix_now, unix_to_rfc3339, EnvironmentSnapshot, Probe, SNAPSHOT_SCHEMA_VERSION,
};
use stacklens_system::{collect_environment_from_os, collect_system};

pub fn current_snapshot() -> EnvironmentSnapshot {
    EnvironmentSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        stacklens_version: env!("CARGO_PKG_VERSION").to_owned(),
        captured_at: unix_to_rfc3339(unix_now()),
        system: Probe::collected(collect_system()),
        environment: Probe::collected(collect_environment_from_os()),
        gpu: Probe::NotImplemented,
        nvidia_driver: Probe::NotImplemented,
        cuda: Probe::NotImplemented,
        libraries: Probe::NotImplemented,
        torch: Probe::NotImplemented,
    }
}
