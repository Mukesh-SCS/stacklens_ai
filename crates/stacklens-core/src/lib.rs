//! Shared domain types for StackLens.
//!
//! This crate is IO-light: it defines the snapshot schema, probe states,
//! findings, and deterministic snapshot diff. Host collection lives in
//! `stacklens-system`.

pub mod diff;
pub mod error;
pub mod finding;
pub mod model;
pub mod probe;
pub mod snapshot;
pub mod time;

pub use diff::{diff_snapshots, ChangeKind, FieldChange, SnapshotDiff};
pub use error::{Error, Result};
pub use finding::{DiagnosticEvidence, DiagnosticFinding, Severity};
pub use model::{
    ContainerDetection, CudaInfo, Endian, EnvironmentInfo, GpuDevice, GpuInfo, LibraryCandidate,
    LibraryInfo, NvidiaDriverInfo, SystemInfo, TorchInfo, WslDetection,
};
pub use probe::Probe;
pub use snapshot::{EnvironmentSnapshot, SNAPSHOT_SCHEMA_VERSION};
pub use time::{unix_now, unix_to_rfc3339};
