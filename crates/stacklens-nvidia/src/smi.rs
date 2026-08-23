use stacklens_core::{GpuDevice, GpuVendor, ProbeIssue, ProbeIssueKind};

use crate::command::{run_nvidia_smi_query, truncate_detail, CommandOutcome, NVIDIA_SMI_TIMEOUT};
use crate::pci::normalize_pci_bus_id;

const INVENTORY_FIELDS: &str = "index,name,uuid,pci.bus_id,memory.total,driver_version";
const COMPUTE_CAP_FIELDS: &str = "uuid,pci.bus_id,compute_cap";
const CUDA_VERSION_FIELDS: &str = "cuda_version";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmiStatus {
    Collected,
    NotPresent,
    Failed(ProbeIssue),
}

#[derive(Debug, Clone)]
pub struct SmiCollection {
    pub devices: Vec<GpuDevice>,
    pub driver_version: Option<String>,
    pub driver_api_cuda_version: Option<String>,
    pub issues: Vec<ProbeIssue>,
    pub status: SmiStatus,
}

#[derive(Debug, Clone, Default)]
pub struct SmiRow {
    pub index: Option<u32>,
    pub name: Option<String>,
    pub uuid: Option<String>,
    pub pci_bus_id: Option<String>,
    pub memory_mib: Option<u64>,
    pub driver_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ComputeCapRow {
    pub uuid: Option<String>,
    pub pci_bus_id: Option<String>,
    pub compute_cap: Option<String>,
}

pub fn collect_nvidia_smi() -> SmiCollection {
    let inventory = query_smi(INVENTORY_FIELDS);
    let mut collection = match classify_query("nvidia-smi", &inventory) {
        QueryClass::Ok(stdout) => match parse_smi_inventory_csv(&stdout) {
            Ok(rows) => smi_from_rows(rows),
            Err(detail) => failed(ProbeIssueKind::Malformed, detail),
        },
        QueryClass::Issue(issue, status) => SmiCollection {
            devices: Vec::new(),
            driver_version: None,
            driver_api_cuda_version: None,
            issues: vec![issue],
            status,
        },
    };

    if !matches!(collection.status, SmiStatus::Collected) {
        return collection;
    }

    let caps = query_smi(COMPUTE_CAP_FIELDS);
    match classify_query("nvidia-smi compute_cap", &caps) {
        QueryClass::Ok(stdout) => match parse_compute_cap_csv(&stdout) {
            Ok(rows) => apply_compute_caps(&mut collection.devices, &rows),
            Err(detail) => collection.issues.push(ProbeIssue {
                source: "nvidia-smi compute_cap".to_owned(),
                kind: ProbeIssueKind::Malformed,
                detail,
            }),
        },
        QueryClass::Issue(issue, _) => collection.issues.push(issue),
    }

    // Optional enrichment. Unsupported query fields must not fail inventory.
    let cuda_api = query_smi(CUDA_VERSION_FIELDS);
    if let QueryClass::Ok(stdout) = classify_query("nvidia-smi cuda_version", &cuda_api) {
        collection.driver_api_cuda_version = parse_cuda_version_csv(&stdout);
    }

    collection
}

fn query_smi(fields: &str) -> CommandOutcome {
    run_nvidia_smi_query(fields)
}

#[derive(Debug)]
enum QueryClass {
    Ok(String),
    Issue(ProbeIssue, SmiStatus),
}

fn classify_query(source: &str, outcome: &CommandOutcome) -> QueryClass {
    if outcome.not_found {
        return QueryClass::Issue(
            ProbeIssue {
                source: source.to_owned(),
                kind: ProbeIssueKind::CommandNotFound,
                detail: outcome
                    .spawn_detail
                    .clone()
                    .unwrap_or_else(|| "nvidia-smi was not found on PATH".to_owned()),
            },
            SmiStatus::NotPresent,
        );
    }
    if outcome.timed_out {
        return QueryClass::Issue(
            ProbeIssue {
                source: source.to_owned(),
                kind: ProbeIssueKind::Timeout,
                detail: format!(
                    "nvidia-smi exceeded {}s timeout",
                    NVIDIA_SMI_TIMEOUT.as_secs()
                ),
            },
            SmiStatus::Failed(ProbeIssue {
                source: source.to_owned(),
                kind: ProbeIssueKind::Timeout,
                detail: format!(
                    "nvidia-smi exceeded {}s timeout",
                    NVIDIA_SMI_TIMEOUT.as_secs()
                ),
            }),
        );
    }
    if let Some(detail) = &outcome.spawn_detail {
        let issue = ProbeIssue {
            source: source.to_owned(),
            kind: ProbeIssueKind::CommandFailed,
            detail: detail.clone(),
        };
        return QueryClass::Issue(issue.clone(), SmiStatus::Failed(issue));
    }
    match outcome.exit_code {
        Some(0) => QueryClass::Ok(outcome.stdout.clone()),
        Some(code) => {
            let detail = if outcome.stderr.trim().is_empty() {
                format!("exit status {code}")
            } else {
                truncate_detail(&outcome.stderr)
            };
            let issue = ProbeIssue {
                source: source.to_owned(),
                kind: ProbeIssueKind::CommandFailed,
                detail,
            };
            QueryClass::Issue(issue.clone(), SmiStatus::Failed(issue))
        }
        None => {
            let issue = ProbeIssue {
                source: source.to_owned(),
                kind: ProbeIssueKind::CommandFailed,
                detail: "nvidia-smi exited without a status".to_owned(),
            };
            QueryClass::Issue(issue.clone(), SmiStatus::Failed(issue))
        }
    }
}

fn smi_from_rows(rows: Vec<SmiRow>) -> SmiCollection {
    let driver_version = common_driver_version(&rows);
    let devices = rows.into_iter().map(row_to_device).collect();
    SmiCollection {
        devices,
        driver_version,
        driver_api_cuda_version: None,
        issues: Vec::new(),
        status: SmiStatus::Collected,
    }
}

fn failed(kind: ProbeIssueKind, detail: String) -> SmiCollection {
    let issue = ProbeIssue {
        source: "nvidia-smi".to_owned(),
        kind,
        detail,
    };
    SmiCollection {
        devices: Vec::new(),
        driver_version: None,
        driver_api_cuda_version: None,
        issues: vec![issue.clone()],
        status: SmiStatus::Failed(issue),
    }
}

fn row_to_device(row: SmiRow) -> GpuDevice {
    GpuDevice {
        vendor: GpuVendor::Nvidia,
        index: row.index,
        name: row.name,
        uuid: row.uuid,
        pci_bus_id: row.pci_bus_id,
        pci_vendor_id: None,
        pci_device_id: None,
        cuda_compute_capability: None,
        memory_bytes: row.memory_mib.map(|mib| mib.saturating_mul(1024 * 1024)),
    }
}

fn common_driver_version(rows: &[SmiRow]) -> Option<String> {
    let mut versions = rows
        .iter()
        .filter_map(|row| row.driver_version.as_deref())
        .collect::<Vec<_>>();
    versions.sort_unstable();
    versions.dedup();
    match versions.as_slice() {
        [version] => Some((*version).to_owned()),
        _ => rows.iter().find_map(|row| row.driver_version.clone()),
    }
}

fn apply_compute_caps(devices: &mut [GpuDevice], rows: &[ComputeCapRow]) {
    for device in devices {
        let cap = rows.iter().find_map(|row| {
            if same_uuid(device.uuid.as_deref(), row.uuid.as_deref())
                || same_pci(device.pci_bus_id.as_deref(), row.pci_bus_id.as_deref())
            {
                row.compute_cap.clone()
            } else {
                None
            }
        });
        if cap.is_some() {
            device.cuda_compute_capability = cap;
        }
    }
}

fn same_uuid(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.eq_ignore_ascii_case(right),
        _ => false,
    }
}

fn same_pci(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

pub fn parse_smi_inventory_csv(text: &str) -> Result<Vec<SmiRow>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|err| format!("CSV parse error on row {index}: {err}"))?;
        if record.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        rows.push(SmiRow {
            index: parse_optional_u32(record.get(0)),
            name: clean_field(record.get(1)),
            uuid: clean_field(record.get(2)),
            pci_bus_id: record.get(3).and_then(normalize_pci_bus_id),
            memory_mib: parse_optional_u64(record.get(4)),
            driver_version: clean_field(record.get(5)),
        });
    }
    Ok(rows)
}

pub fn parse_cuda_version_csv(text: &str) -> Option<String> {
    for line in text.lines() {
        let field = line.split(',').next().unwrap_or(line);
        if let Some(value) = clean_field(Some(field)) {
            return Some(value);
        }
    }
    None
}

pub fn parse_compute_cap_csv(text: &str) -> Result<Vec<ComputeCapRow>, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|err| format!("CSV parse error on row {index}: {err}"))?;
        rows.push(ComputeCapRow {
            uuid: clean_field(record.get(0)),
            pci_bus_id: record.get(1).and_then(normalize_pci_bus_id),
            compute_cap: clean_field(record.get(2)),
        });
    }
    Ok(rows)
}

fn clean_field(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("[n/a]") || value.eq_ignore_ascii_case("n/a")
    {
        None
    } else {
        Some(value.to_owned())
    }
}

fn parse_optional_u32(value: Option<&str>) -> Option<u32> {
    clean_field(value)?.parse().ok()
}

fn parse_optional_u64(value: Option<&str>) -> Option<u64> {
    clean_field(value)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{
        classify_query, parse_compute_cap_csv, parse_cuda_version_csv, parse_smi_inventory_csv,
        QueryClass, SmiStatus,
    };
    use crate::command::CommandOutcome;
    use stacklens_core::ProbeIssueKind;

    #[test]
    fn single_gpu_csv() {
        let rows = parse_smi_inventory_csv(
            "0, NVIDIA GeForce RTX 4090, GPU-11111111-1111-1111-1111-111111111111, 00000000:01:00.0, 24564, 580.82.07\n",
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].index, Some(0));
        assert_eq!(rows[0].name.as_deref(), Some("NVIDIA GeForce RTX 4090"));
        assert_eq!(rows[0].memory_mib, Some(24564));
        assert_eq!(rows[0].driver_version.as_deref(), Some("580.82.07"));
        assert_eq!(rows[0].pci_bus_id.as_deref(), Some("00000000:01:00.0"));
    }

    #[test]
    fn multiple_gpu_csv() {
        let text = "\
0, NVIDIA A100-SXM4-80GB, GPU-aaaa, 00000000:01:00.0, 81920, 570.124.04
1, NVIDIA A100-SXM4-80GB, GPU-bbbb, 00000000:21:00.0, 81920, 570.124.04
";
        let rows = parse_smi_inventory_csv(text).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].uuid.as_deref(), Some("GPU-bbbb"));
    }

    #[test]
    fn quoted_name_with_comma() {
        let rows = parse_smi_inventory_csv(
            r#"0,"NVIDIA GeForce RTX 4090, 24GB",GPU-1234,0000:01:00.0,24564,580.82.07"#,
        )
        .unwrap();
        assert_eq!(
            rows[0].name.as_deref(),
            Some("NVIDIA GeForce RTX 4090, 24GB")
        );
        assert_eq!(rows[0].uuid.as_deref(), Some("GPU-1234"));
    }

    #[test]
    fn cuda_version_csv() {
        assert_eq!(parse_cuda_version_csv("13.0\n").as_deref(), Some("13.0"));
        assert_eq!(parse_cuda_version_csv(" 12.8 \n").as_deref(), Some("12.8"));
        assert_eq!(parse_cuda_version_csv("[N/A]\n"), None);
    }

    #[test]
    fn compute_cap_csv() {
        let rows = parse_compute_cap_csv(
            "GPU-11111111-1111-1111-1111-111111111111, 00000000:01:00.0, 8.9\n",
        )
        .unwrap();
        assert_eq!(rows[0].compute_cap.as_deref(), Some("8.9"));
    }

    #[test]
    fn command_not_found_mapping() {
        match classify_query(
            "nvidia-smi",
            &CommandOutcome {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                timed_out: false,
                not_found: true,
                spawn_detail: Some("program not found".to_owned()),
            },
        ) {
            QueryClass::Issue(issue, SmiStatus::NotPresent) => {
                assert_eq!(issue.kind, ProbeIssueKind::CommandNotFound);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn nonzero_status_mapping() {
        match classify_query(
            "nvidia-smi",
            &CommandOutcome {
                exit_code: Some(255),
                stdout: String::new(),
                stderr: "Failed to initialize NVML: Driver/library version mismatch\n".to_owned(),
                timed_out: false,
                not_found: false,
                spawn_detail: None,
            },
        ) {
            QueryClass::Issue(issue, SmiStatus::Failed(_)) => {
                assert_eq!(issue.kind, ProbeIssueKind::CommandFailed);
                assert!(issue.detail.contains("Driver/library version mismatch"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
