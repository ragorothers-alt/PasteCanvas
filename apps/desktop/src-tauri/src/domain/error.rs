use thiserror::Error;

/// Domain-level errors that represent business rule violations.
/// These are independent of infrastructure or framework concerns.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid revision: {0}, must be >= 1")]
    InvalidRevision(i64),

    #[error("title too long: {0} bytes, max 2000")]
    TitleTooLong(usize),

    #[error("board cannot be its own ancestor")]
    BoardCycleDetected,

    #[error("group parent cycle detected")]
    GroupCycleDetected,

    #[error("unsupported item type: {0}")]
    UnsupportedItemType(String),

    #[error("content too large: {size} bytes exceeds limit {limit}")]
    ContentTooLarge { size: u64, limit: u64 },
}
