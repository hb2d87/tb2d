use crate::layout::{FocusRef, ViewportState};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub focus: FocusRef,
    #[serde(default)]
    pub viewport: ViewportState,
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    pub fn new(name: String) -> Self {
        let root = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self { path: root.join("tb2d").join(format!("{name}.json")) }
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<SessionState> {
        if !self.path.exists() {
            return Ok(SessionState::default());
        }
        let source = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read session {}", self.path.display()))?;
        serde_json::from_str(&source).context("invalid session JSON")
    }

    pub fn save(&self, state: &SessionState) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(state)?;
        fs::write(&self.path, json)
            .with_context(|| format!("failed to save session {}", self.path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session_state() {
        let path = std::env::temp_dir().join(format!("tb2d-session-{}.json", std::process::id()));
        let store = SessionStore::at(path.clone());
        let expected = SessionState {
            focus: FocusRef { column: 2, pane: 1 },
            viewport: ViewportState { offset: 42 },
        };
        store.save(&expected).unwrap();
        assert_eq!(store.load().unwrap(), expected);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn missing_session_uses_defaults() {
        let path = std::env::temp_dir().join(format!("tb2d-missing-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        assert_eq!(SessionStore::at(path).load().unwrap(), SessionState::default());
    }
}
