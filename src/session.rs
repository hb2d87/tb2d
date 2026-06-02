use crate::{
    app::PaneViewState,
    layout::{FocusRef, ViewportState},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    pub template: Option<PathBuf>,
    #[serde(default)]
    pub focus: FocusRef,
    #[serde(default)]
    pub viewport: ViewportState,
    #[serde(default, deserialize_with = "deserialize_column_widths")]
    pub column_widths: Vec<Option<u16>>,
    #[serde(default, deserialize_with = "deserialize_pane_selections")]
    pub pane_selections: Vec<usize>,
    #[serde(default, deserialize_with = "deserialize_pane_views")]
    pub pane_views: Vec<Vec<PaneViewState>>,
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
        Self { path: root.join("tb2d").join(session_filename(&name)) }
    }

    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn diagnostics_path(&self) -> PathBuf {
        let parent = self.path.parent().unwrap_or_else(|| Path::new("."));
        let stem = self
            .path
            .file_stem()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| "session".into());
        parent.join(format!("{stem}.diagnostics.jsonl"))
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
        let (temporary, mut file) = self.open_temporary_file()?;
        file.write_all(json.as_bytes())
            .with_context(|| format!("failed to write session {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("failed to flush session {}", temporary.display()))?;
        drop(file);
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| format!("failed to save session {}", self.path.display()));
        }
        Ok(())
    }

    pub fn append_diagnostic(&self, event: &str, fields: &[(&str, Value)]) -> Result<()> {
        let path = self.diagnostics_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let mut record = Map::new();
        record.insert("ts_unix_ms".to_owned(), Value::from(unix_millis()));
        record.insert("event".to_owned(), Value::from(event));
        for (key, value) in fields {
            record.insert((*key).to_owned(), value.clone());
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("failed to open diagnostics log {}", path.display()))?;
        serde_json::to_writer(&mut file, &Value::Object(record))?;
        file.write_all(b"\n")
            .with_context(|| format!("failed to write diagnostics log {}", path.display()))?;
        Ok(())
    }

    fn open_temporary_file(&self) -> Result<(PathBuf, std::fs::File)> {
        let mut attempt = 0_u32;
        loop {
            let temporary = temporary_path(&self.path, attempt);
            match OpenOptions::new().create_new(true).write(true).open(&temporary) {
                Ok(file) => return Ok((temporary, file)),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    attempt = attempt.saturating_add(1);
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("failed to create {}", temporary.display()));
                }
            }
        }
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn deserialize_column_widths<'de, D>(deserializer: D) -> std::result::Result<Vec<Option<u16>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_vec_or_default(deserializer)
}

fn deserialize_pane_selections<'de, D>(deserializer: D) -> std::result::Result<Vec<usize>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_vec_or_default(deserializer)
}

fn deserialize_pane_views<'de, D>(deserializer: D) -> std::result::Result<Vec<Vec<PaneViewState>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_vec_or_default(deserializer)
}

fn deserialize_vec_or_default<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer).ok().flatten().unwrap_or_default())
}

fn temporary_path(path: &Path, attempt: u32) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let base = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "session.json".into());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    parent.join(format!(".{base}.{pid}.{nanos}.{attempt}.tmp", pid = std::process::id()))
}

fn session_filename(name: &str) -> String {
    let normalized = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{}.json", if normalized.is_empty() { "main" } else { &normalized })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_loads_session_state() {
        let path = std::env::temp_dir().join(format!("tb2d-session-{}.json", std::process::id()));
        let store = SessionStore::at(path.clone());
        let expected = SessionState {
            template: Some(PathBuf::from("/tmp/workspace.yaml")),
            focus: FocusRef { column: 2, pane: 1 },
            viewport: ViewportState { offset: 42 },
            column_widths: vec![None, Some(72), None],
            pane_selections: vec![1, 0, 2],
            pane_views: vec![vec![PaneViewState::default()]],
        };
        store.save(&expected).unwrap();
        assert_eq!(store.load().unwrap(), expected);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn appends_jsonl_diagnostics_next_to_session() {
        let path = std::env::temp_dir().join(format!("tb2d-diagnostics-{}.json", std::process::id()));
        let store = SessionStore::at(path.clone());
        let diagnostics = store.diagnostics_path();
        let _ = fs::remove_file(&diagnostics);

        store
            .append_diagnostic("test-event", &[("answer", Value::from(42))])
            .unwrap();

        let log = fs::read_to_string(&diagnostics).unwrap();
        assert!(log.contains(r#""event":"test-event""#));
        assert!(log.contains(r#""answer":42"#));
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(diagnostics);
    }

    #[test]
    fn missing_session_uses_defaults() {
        let path = std::env::temp_dir().join(format!("tb2d-missing-{}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        assert_eq!(SessionStore::at(path).load().unwrap(), SessionState::default());
    }

    #[test]
    fn tolerates_missing_and_malformed_width_arrays() {
        let missing: SessionState = serde_json::from_str(
            r#"{"focus":{"column":1,"pane":0},"viewport":{"offset":12}}"#,
        ).unwrap();
        let malformed: SessionState = serde_json::from_str(
            r#"{"focus":{"column":1,"pane":0},"viewport":{"offset":12},"column_widths":"oops","pane_selections":"oops","pane_views":"oops"}"#,
        ).unwrap();
        assert_eq!(missing.column_widths, Vec::<Option<u16>>::new());
        assert_eq!(malformed.column_widths, Vec::<Option<u16>>::new());
        assert_eq!(missing.pane_selections, Vec::<usize>::new());
        assert_eq!(missing.pane_views, Vec::<Vec<PaneViewState>>::new());
        assert_eq!(malformed.pane_selections, Vec::<usize>::new());
        assert_eq!(malformed.pane_views, Vec::<Vec<PaneViewState>>::new());
    }

    #[test]
    fn older_sessions_without_a_template_remain_compatible() {
        let state: SessionState = serde_json::from_str(
            r#"{"focus":{"column":1,"pane":0},"viewport":{"offset":12}}"#,
        ).unwrap();
        assert_eq!(state.template, None);
    }

    #[test]
    fn session_names_cannot_escape_the_state_directory() {
        assert_eq!(session_filename("../demo workspace"), "___demo_workspace.json");
        assert_eq!(session_filename(""), "main.json");
    }
}
