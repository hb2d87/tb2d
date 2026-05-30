use thiserror::Error;

#[derive(Debug, Error)]
pub enum Tb2dError {
    #[error("workspace must define at least one column")]
    EmptyWorkspace,
    #[error("column {0:?} must define at least one pane")]
    EmptyColumn(String),
    #[error("unknown width preset {0:?}")]
    UnknownPreset(String),
    #[error("invalid width policy {0:?}")]
    InvalidWidth(String),
}
