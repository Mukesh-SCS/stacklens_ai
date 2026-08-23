use stacklens_core::CudaMajorFamilyEvidence;

/// Broad minimum driver *branch* for CUDA major-family minor-version
/// compatibility, as documented by NVIDIA. This is not a per-release
/// compatibility matrix and does not imply full feature support, PTX JIT
/// success, or architecture coverage.
///
/// Source: NVIDIA CUDA Compatibility documentation
/// https://docs.nvidia.com/deploy/cuda-compatibility/
/// and CUDA Toolkit release notes.
/// Reviewed against public NVIDIA documentation, August 2026.
///
/// Approximate documented branches:
/// - CUDA 11.x: driver branch >= 450
/// - CUDA 12.x: driver branch >= 525
/// - CUDA 13.x: driver branch >= 580
pub const MAJOR_FAMILY_MIN_DRIVER_BRANCH: &[(u16, u16)] = &[(11, 450), (12, 525), (13, 580)];

pub const COMPAT_TABLE_SOURCE: &str =
    "NVIDIA CUDA Compatibility documentation (reviewed August 2026)";

pub fn parse_driver_branch(version: &str) -> Option<u16> {
    let first = version.trim().split('.').next()?;
    if first.is_empty() || !first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    first.parse().ok()
}

pub fn evidence_for_majors(
    majors: &[u16],
    driver_version: Option<&str>,
) -> Vec<CudaMajorFamilyEvidence> {
    let observed = driver_version.and_then(parse_driver_branch);
    let mut items = Vec::new();
    let mut seen = Vec::new();
    for major in majors {
        if seen.contains(major) {
            continue;
        }
        seen.push(*major);
        if let Some(minimum) = minimum_branch_for(*major) {
            items.push(family_evidence(*major, minimum, observed));
        }
    }
    items.sort_by_key(|item| item.cuda_major);
    items
}

pub fn minimum_branch_for(major: u16) -> Option<u16> {
    MAJOR_FAMILY_MIN_DRIVER_BRANCH
        .iter()
        .find(|(family, _)| *family == major)
        .map(|(_, branch)| *branch)
}

fn family_evidence(
    cuda_major: u16,
    minimum: u16,
    observed: Option<u16>,
) -> CudaMajorFamilyEvidence {
    let meets = observed.map(|branch| branch >= minimum);
    let note = match (observed, meets) {
        (Some(branch), Some(true)) => format!(
            "observed driver branch {branch} meets the documented CUDA {cuda_major}.x minor-compatibility minimum of {minimum}; this is not a full compatibility determination"
        ),
        (Some(branch), Some(false)) => format!(
            "observed driver branch {branch} is below the documented CUDA {cuda_major}.x minor-compatibility minimum of {minimum}"
        ),
        _ => format!(
            "documented CUDA {cuda_major}.x minor-compatibility minimum driver branch is {minimum}; observed driver branch was not available"
        ),
    };
    CudaMajorFamilyEvidence {
        cuda_major,
        documented_minimum_driver_branch: minimum,
        observed_driver_branch: observed,
        driver_branch_meets_documented_minimum: meets,
        source: COMPAT_TABLE_SOURCE.to_owned(),
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::{evidence_for_majors, parse_driver_branch};

    #[test]
    fn parses_driver_branch() {
        assert_eq!(parse_driver_branch("580.82.07"), Some(580));
        assert_eq!(parse_driver_branch("525.60.13"), Some(525));
        assert_eq!(parse_driver_branch("not-a-version"), None);
    }

    #[test]
    fn twelve_family_minimum_is_neutral() {
        let items = evidence_for_majors(&[12], Some("580.82.07"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].documented_minimum_driver_branch, 525);
        assert_eq!(items[0].driver_branch_meets_documented_minimum, Some(true));
        assert!(!items[0].note.contains("fully"));
        assert!(items[0]
            .note
            .contains("not a full compatibility determination"));
    }
}
