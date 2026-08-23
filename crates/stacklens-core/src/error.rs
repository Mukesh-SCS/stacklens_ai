use std::io;

/// Application-level failures. Missing GPUs or toolkits are [`crate::Probe`]
/// states, not errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("failed to parse snapshot: {0}")]
    SnapshotParse(#[from] serde_json::Error),

    #[error("unsupported snapshot schema version {found} (this build supports {supported})")]
    UnsupportedSchema { found: u32, supported: u32 },
}

pub type Result<T> = std::result::Result<T, Error>;
