use thiserror::Error;

#[derive(Debug, Error)]
pub enum Tb2dError {
    #[error("workspace must define at least one column")]
    EmptyWorkspace,
    #[error("column names must not be empty")]
    EmptyColumnName,
    #[error("workspace contains duplicate column name {0:?}")]
    DuplicateColumn(String),
    #[error("column {0:?} must define at least one pane")]
    EmptyColumn(String),
    #[error("pane names in column {0:?} must not be empty")]
    EmptyPaneName(String),
    #[error("column {column:?} contains duplicate pane name {pane:?}")]
    DuplicatePane { column: String, pane: String },
    #[error("pane {pane:?} in column {column:?} must define a non-empty command")]
    EmptyCommand { column: String, pane: String },
    #[error("unknown width preset {0:?}")]
    UnknownPreset(String),
    #[error("width preset {0:?} must be greater than zero")]
    ZeroPreset(String),
    #[error("invalid width policy {0:?}")]
    InvalidWidth(String),
}
