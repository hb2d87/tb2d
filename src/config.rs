use crate::error::Tb2dError;
use anyhow::{Context, Result};
use ratatui::style::Color;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::{collections::{HashMap, HashSet}, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    #[serde(default = "default_workspace_name")]
    pub name: String,
    #[serde(default = "default_gap")]
    pub gap: u16,
    #[serde(default = "default_peek")]
    pub peek: u16,
    #[serde(default)]
    pub wrap_columns: bool,
    #[serde(default)]
    pub presets: WidthPresets,
    #[serde(default)]
    pub ui: UiConfig,
    pub columns: Vec<ColumnConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnConfig {
    pub name: String,
    #[serde(default)]
    pub layout: PaneLayoutMode,
    pub width: WidthPolicy,
    pub panes: Vec<PaneConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneLayoutMode {
    Fit,
    Tabs,
    Carousel,
}

impl Default for PaneLayoutMode {
    fn default() -> Self {
        Self::Fit
    }
}

impl PaneLayoutMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::Tabs => "tabs",
            Self::Carousel => "carousel",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub accent: UiColor,
    pub muted: UiColor,
    pub selection_fg: UiColor,
    pub selection_bg: UiColor,
    pub status_fg: UiColor,
    pub status_bg: UiColor,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            accent: UiColor::LightMagenta,
            muted: UiColor::DarkGray,
            selection_fg: UiColor::Black,
            selection_bg: UiColor::White,
            status_fg: UiColor::White,
            status_bg: UiColor::Blue,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UiColor {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
}

impl UiColor {
    pub fn to_color(self) -> Color {
        match self {
            Self::Reset => Color::Reset,
            Self::Black => Color::Black,
            Self::Red => Color::Red,
            Self::Green => Color::Green,
            Self::Yellow => Color::Yellow,
            Self::Blue => Color::Blue,
            Self::Magenta => Color::Magenta,
            Self::Cyan => Color::Cyan,
            Self::Gray => Color::Gray,
            Self::DarkGray => Color::DarkGray,
            Self::LightRed => Color::LightRed,
            Self::LightGreen => Color::LightGreen,
            Self::LightYellow => Color::LightYellow,
            Self::LightBlue => Color::LightBlue,
            Self::LightMagenta => Color::LightMagenta,
            Self::LightCyan => Color::LightCyan,
            Self::White => Color::White,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneConfig {
    pub name: String,
    #[serde(default = "default_command")]
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub fn default_template() -> Result<Self> {
        Self::parse(include_str!("../examples/default.yaml"))
            .context("invalid built-in default workspace")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .with_context(|| format!("failed to read workspace {}", path.display()))?;
        Self::parse(&source)
            .with_context(|| format!("failed to parse workspace {}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let workspace: Self = serde_yaml::from_str(source).context("invalid workspace YAML")?;
        workspace.validate()?;
        Ok(workspace)
    }

    pub fn validate(&self) -> Result<()> {
        if self.columns.is_empty() {
            return Err(Tb2dError::EmptyWorkspace.into());
        }
        self.presets.validate()?;
        let mut column_names = HashSet::new();
        for column in &self.columns {
            if column.name.trim().is_empty() {
                return Err(Tb2dError::EmptyColumnName.into());
            }
            if !column_names.insert(&column.name) {
                return Err(Tb2dError::DuplicateColumn(column.name.clone()).into());
            }
            if column.panes.is_empty() {
                return Err(Tb2dError::EmptyColumn(column.name.clone()).into());
            }
            column.width.resolve(100, &self.presets)?;
            let mut pane_names = HashSet::new();
            for pane in &column.panes {
                if pane.name.trim().is_empty() {
                    return Err(Tb2dError::EmptyPaneName(column.name.clone()).into());
                }
                if !pane_names.insert(&pane.name) {
                    return Err(Tb2dError::DuplicatePane {
                        column: column.name.clone(),
                        pane: pane.name.clone(),
                    }.into());
                }
                if pane.command.trim().is_empty() {
                    return Err(Tb2dError::EmptyCommand {
                        column: column.name.clone(),
                        pane: pane.name.clone(),
                    }.into());
                }
            }
        }
        Ok(())
    }
}

impl WidthPolicy {
    pub fn parse(value: &str) -> Result<Self, Tb2dError> {
        let value = value.trim();
        if let Ok(cells) = value.parse::<u16>() {
            return (cells > 0)
                .then_some(Self::Cells(cells))
                .ok_or_else(|| Tb2dError::InvalidWidth(value.to_owned()));
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
                "min" if min.is_none() && cells > 0 => min = Some(cells),
                "max" if max.is_none() && cells > 0 => max = Some(cells),
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
                cells.max(1)
            }
        };
        (cells > 0)
            .then_some(cells)
            .ok_or_else(|| Tb2dError::InvalidWidth(format!("{self:?}")))
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

    fn validate(&self) -> Result<()> {
        for (name, width) in [
            ("small", self.small),
            ("medium", self.medium),
            ("big", self.big),
        ].into_iter().chain(self.extra.iter().map(|(name, width)| (name.as_str(), *width))) {
            if width == 0 {
                return Err(Tb2dError::ZeroPreset(name.to_owned()).into());
            }
        }
        Ok(())
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
                let width = u16::try_from(value)
                    .map_err(|_| E::custom("width cell count is too large"))?;
                (width > 0)
                    .then_some(WidthPolicy::Cells(width))
                    .ok_or_else(|| E::custom("width cell count must be greater than zero"))
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

impl Serialize for WidthPolicy {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Cells(cells) => serializer.serialize_u16(*cells),
            Self::Preset(preset) => serializer.serialize_str(preset),
            Self::Percent { percent, min, max } => {
                let mut value = format!("{percent}%");
                if let Some(min) = min {
                    value.push_str(&format!(" min={min}"));
                }
                if let Some(max) = max {
                    value.push_str(&format!(" max={max}"));
                }
                serializer.serialize_str(&value)
            }
        }
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
        assert_eq!(WidthPolicy::parse("1%").unwrap().resolve(20, &presets).unwrap(), 1);
    }

    #[test]
    fn rejects_invalid_percentage() {
        assert!(WidthPolicy::parse("101%").is_err());
        assert!(WidthPolicy::parse("50% min=90 max=30").is_err());
        assert!(WidthPolicy::parse("50% min=20 min=30").is_err());
        assert!(WidthPolicy::parse("0").is_err());
    }

    #[test]
    fn parses_yaml_workspace_with_defaults() {
        let workspace = Workspace::parse(
            "columns:\n  - name: editor\n    width: medium\n    panes:\n      - name: shell\n        command: echo hi\n",
        ).unwrap();
        assert_eq!(workspace.gap, 2);
        assert_eq!(workspace.peek, 3);
        assert!(!workspace.wrap_columns);
        assert_eq!(workspace.columns[0].width, WidthPolicy::Preset("medium".into()));
        assert_eq!(workspace.columns[0].layout, PaneLayoutMode::Fit);
        assert_eq!(workspace.ui.accent, UiColor::LightMagenta);
    }

    #[test]
    fn parses_explicit_ui_and_carousel_layout() {
        let workspace = Workspace::parse(
            "wrap_columns: true\nui:\n  accent: light-magenta\n  muted: dark-gray\n  selection_fg: black\n  selection_bg: white\n  status_fg: white\n  status_bg: blue\ncolumns:\n  - name: editor\n    layout: carousel\n    width: medium\n    panes:\n      - name: shell\n        command: echo hi\n",
        ).unwrap();
        assert!(workspace.wrap_columns);
        assert_eq!(workspace.ui.accent, UiColor::LightMagenta);
        assert_eq!(workspace.ui.selection_bg, UiColor::White);
        assert_eq!(workspace.ui.status_bg, UiColor::Blue);
        assert_eq!(workspace.columns[0].layout, PaneLayoutMode::Carousel);
    }

    #[test]
    fn rejects_empty_columns() {
        assert!(Workspace::parse("columns: []").is_err());
        assert!(Workspace::parse("columns:\n  - name: bad\n    width: 40\n    panes: []").is_err());
    }

    #[test]
    fn rejects_ambiguous_or_empty_workspace_values() {
        assert!(Workspace::parse(
            "columns:\n  - name: one\n    width: 0\n    panes:\n      - name: shell\n",
        ).is_err());
        assert!(Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: shell\n        command: ''\n",
        ).is_err());
        assert!(Workspace::parse(
            "columns:\n  - name: one\n    width: 40\n    panes:\n      - name: shell\n  - name: one\n    width: 40\n    panes:\n      - name: shell\n",
        ).is_err());
    }

    #[test]
    fn rejects_unknown_workspace_fields() {
        assert!(Workspace::parse(
            "unexpected: true\ncolumns:\n  - name: one\n    width: 40\n    panes:\n      - name: shell\n",
        ).is_err());
    }

    #[test]
    fn built_in_default_workspace_has_the_product_layout() {
        let workspace = Workspace::default_template().unwrap();
        assert_eq!(workspace.columns.len(), 4);
        assert_eq!(workspace.columns[0].name, "welcome");
        assert_eq!(workspace.columns[0].layout, PaneLayoutMode::Fit);
        assert_eq!(workspace.columns[0].panes.len(), 2);
        assert_eq!(workspace.columns[0].panes[0].name, "welcome");
        assert!(workspace.columns[0].panes[0]
            .command
            .contains("Welcome to your terminal workspace."));
        assert!(!workspace.columns[0].panes[0].command.contains("README.md"));
        assert_eq!(workspace.columns[1].name, "main");
        assert_eq!(workspace.columns[1].width, WidthPolicy::Preset("big".into()));
        assert_eq!(workspace.columns[2].name, "carousel");
        assert_eq!(workspace.columns[2].layout, PaneLayoutMode::Carousel);
        assert_eq!(workspace.columns[2].panes.len(), 3);
        assert_eq!(workspace.columns[3].name, "Agent");
        assert_eq!(workspace.columns[3].width, WidthPolicy::Preset("big".into()));
    }
}
