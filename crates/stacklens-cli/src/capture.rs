use stacklens_core::{
    unix_now, unix_to_rfc3339, CudaVersion, DriverApiCapability, EnvironmentSnapshot, Probe,
    SNAPSHOT_SCHEMA_VERSION,
};
use stacklens_system::{collect_environment_from_os, collect_system};

pub fn current_snapshot() -> EnvironmentSnapshot {
    let system = collect_system();
    let environment = collect_environment_from_os();
    let nvidia = stacklens_nvidia::inspect();
    let driver_version = nvidia.driver.as_collected().and_then(|info| {
        info.version
            .clone()
            .or_else(|| info.userspace_version.clone())
            .or_else(|| info.kernel_module_version.clone())
    });
    let driver_api_capability =
        nvidia
            .driver_api_capability
            .as_ref()
            .map(|raw| DriverApiCapability {
                raw: raw.clone(),
                version: CudaVersion::parse(raw),
                source: "nvidia-smi".to_owned(),
            });
    let cuda =
        stacklens_cuda::inspect_live(&environment, &system, driver_api_capability, driver_version);

    EnvironmentSnapshot {
        schema_version: SNAPSHOT_SCHEMA_VERSION,
        stacklens_version: env!("CARGO_PKG_VERSION").to_owned(),
        captured_at: unix_to_rfc3339(unix_now()),
        system: Probe::collected(system),
        environment: Probe::collected(environment),
        gpu: nvidia.gpu,
        nvidia_driver: nvidia.driver,
        cuda,
        libraries: Probe::NotImplemented,
        torch: Probe::NotImplemented,
    }
}
