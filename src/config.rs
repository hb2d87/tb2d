use crate::error::Tb2dError;
use anyhow::{Context, Result};
use serde::{de, Deserialize, Deserializer};
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    #[serde(default = "default_workspace_name")]
    pub name: String,
    #[serde(default = "default_gap")]
    pub gap: u16,
    #[serde(default = "default_peek")]
    pub peek: u16,
    #[serde(default)]
    pub presets: WidthPresets,
    pub columns: Vec<ColumnConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ColumnConfig {
    pub name: String,
    pub width: WidthPolicy,
    pub panes: Vec<PaneConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaneConfig {
    pub name: String,
    #[serde(default = "default_command")]
    pub command: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WidthPresets {
    #[serde(default = "default_small")]
    pub small: u16,
    #[serde(default = "default_medium")]
    pub medium: u16,
    #[serde(default = "default_big")]
    pub big: u16,
    #[serde(flatten)]
    pub extra: HashMap<String, u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidthPolicy {
    Cells(u16),
    Percent {
        percent: u16,
        min: Option<u16>,
        max: Option<u16>,
    },
    Preset(String),
}

impl Workspace {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read workspace {}", path.display()))?;
        Self::parse(&source)
    }

    pub fn parse(source: &str) -> Result<Self> {
        let workspace: Self = serde_yaml::from_str(source).context("invalid workspace YAML")?;
        workspace.validate()?;
        Ok(workspace)
    }

    fn validate(&self) -> Result<()> {
        if self.columns.is_empty() {
            return Err(Tb2dError::EmptyWorkspace.into());
        }
        for column in &self.columns {
            if column.panes.is_empty() {
                return Err(Tb2dError::EmptyColumn(column.name.clone()).into());
            }
            column.width.resolve(100, &self.presets)?;
        }
        Ok(())
    }
}

impl WidthPolicy {
    pub fn parse(value: &str) -> Result<Self, Tb2dError> {
        let value = value.trim();
        if let Ok(cells) = value.parse::<u16>() {
            return Ok(Self::Cells(cells));
        }
        if !value.contains('%') {
            return Ok(Self::Preset(value.to_owned()));
        }

        let mut parts = value.split_whitespace();
        let percent = parts
            .next()
            .and_then(|part| part.strip_suffix('%'))
            .and_then(|part| part.parse::<u16>().ok())
            .filter(|percent| *percent > 0 && *percent <= 100)
            .ok_or_else(|| Tb2dError::InvalidWidth(value.to_owned()))?;
        let mut min = None;
        let mut max = None;
        for part in parts {
            let (key, raw) = part
                .split_once('=')
                .ok_or_else(|| Tb2dError::InvalidWidth(value.to_owned()))?;
            let cells = raw
                .parse::<u16>()
                .map_err(|_| Tb2dError::InvalidWidth(value.to_owned()))?;
            match key {
                "min" => min = Some(cells),
                "max" => max = Some(cells),
                _ => return Err(Tb2dError::InvalidWidth(value.to_owned())),
            }
        }
        if min.zip(max).is_some_and(|(min, max)| min > max) {
            return Err(Tb2dError::InvalidWidth(value.to_owned()));
        }
        Ok(Self::Percent { percent, min, max })
    }

    pub fn resolve(&self, viewport_width: u16, presets: &WidthPresets) -> Result<u16, Tb2dError> {
        let cells = match self {
            Self::Cells(cells) => *cells,
            Self::Preset(name) => presets
                .get(name)
                .ok_or_else(|| Tb2dError::UnknownPreset(name.clone()))?,
            Self::Percent { percent, min, max } => {
                let mut cells = viewport_width.saturating_mul(*percent) / 100;
                if let Some(min) = min {
                    cells = cells.max(*min);
                }
                if let Some(max) = max {
                    cells = cells.min(*max);
                }
                cells
            }
        };
        Ok(cells.max(1))
    }
}

impl WidthPresets {
    pub fn get(&self, name: &str) -> Option<u16> {
        match name {
            "small" => Some(self.small),
            "medium" => Some(self.medium),
            "big" => Some(self.big),
            other => self.extra.get(other).copied(),
        }
    }
}

impl Default for WidthPresets {
    fn default() -> Self {
        Self {
            small: default_small(),
            medium: default_medium(),
            big: default_big(),
            extra: HashMap::new(),
        }
    }
}

impl<'de> Deserialize<'de> for WidthPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = WidthPolicy;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a cell count, preset name, or percentage width")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u16::try_from(value)
                    .map(WidthPolicy::Cells)
                    .map_err(|_| E::custom("width cell count is too large"))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                WidthPolicy::parse(value).map_err(E::custom)
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}

fn default_workspace_name() -> String { "workspace".to_owned() }
fn default_command() -> String { std::env::var("SHELL").unwrap_or_else(|_| "sh".to_owned()) }
fn default_gap() -> u16 { 2 }
fn default_peek() -> u16 { 3 }
fn default_small() -> u16 { 36 }
fn default_medium() -> u16 { 56 }
fn default_big() -> u16 { 80 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_width_policies() {
        assert_eq!(WidthPolicy::parse("72").unwrap(), WidthPolicy::Cells(72));
        assert_eq!(WidthPolicy::parse("big").unwrap(), WidthPolicy::Preset("big".into()));
        assert_eq!(
            WidthPolicy::parse("50% min=30 max=90").unwrap(),
            WidthPolicy::Percent { percent: 50, min: Some(30), max: Some(90) }
        );
    }

    #[test]
    fn resolves_presets_and_clamped_percentages() {
        let presets = WidthPresets::default();
        assert_eq!(WidthPolicy::Preset("medium".into()).resolve(120, &presets).unwrap(), 56);
        assert_eq!(WidthPolicy::parse("50% min=30 max=90").unwrap().resolve(10, &presets).unwrap(), 30);
        assert_eq!(WidthPolicy::parse("50% min=30 max=90").unwrap().resolve(240, &presets).unwrap(), 90);
    }

    #[test]
    fn rejects_invalid_percentage() {
        assert!(WidthPolicy::parse("101%").is_err());
        assert!(WidthPolicy::parse("50% min=90 max=30").is_err());
    }

    #[test]
    fn parses_yaml_workspace_with_defaults() {
        let workspace = Workspace::parse(
            "columns:\n  - name: editor\n    width: medium\n    panes:\n      - name: shell\n        command: echo hi\n",
        ).unwrap();
        assert_eq!(workspace.gap, 2);
        assert_eq!(workspace.peek, 3);
        assert_eq!(workspace.columns[0].width, WidthPolicy::Preset("medium".into()));
    }

    #[test]
    fn rejects_empty_columns() {
        assert!(Workspace::parse("columns: []").is_err());
        assert!(Workspace::parse("columns:\n  - name: bad\n    width: 40\n    panes: []").is_err());
    }
}
