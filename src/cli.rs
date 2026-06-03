use anyhow::{bail, Result};
use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cli {
    /// YAML workspace template. When omitted, restore the session template or use the built-in default.
    pub template: Option<PathBuf>,

    /// Session name used for persisted focus and viewport state
    pub session: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    Run(Cli),
    EditConfig,
    PrintConfigPath,
    Help,
    Version,
}

impl Cli {
    pub fn parse() -> Result<ParseOutcome> {
        Self::parse_from(std::env::args_os().skip(1))
    }

    pub fn parse_from<I, T>(args: I) -> Result<ParseOutcome>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        let mut args = args.into_iter().map(Into::into);
        let mut template = None;
        let mut session = String::from("main");
        let mut config_action = None;

        while let Some(arg) = args.next() {
            let arg = arg.to_string_lossy();
            match arg.as_ref() {
                "-h" | "--help" => return Ok(ParseOutcome::Help),
                "-V" | "--version" => return Ok(ParseOutcome::Version),
                "--config" => {
                    set_config_action(&mut config_action, ParseOutcome::EditConfig)?;
                }
                "--config-path" => {
                    set_config_action(&mut config_action, ParseOutcome::PrintConfigPath)?;
                }
                "--template" => {
                    let Some(value) = args.next() else {
                        bail!("--template requires a path");
                    };
                    set_template(&mut template, PathBuf::from(value))?;
                }
                value if value.starts_with("--template=") => {
                    set_template(&mut template, PathBuf::from(&value[11..]))?;
                }
                "--session" => {
                    let Some(value) = args.next() else {
                        bail!("--session requires a name");
                    };
                    session = value.to_string_lossy().into_owned();
                }
                value if value.starts_with("--session=") => {
                    session = value[10..].to_owned();
                }
                value if value.starts_with('-') => {
                    bail!("unknown option {value:?}; run tb2d --help for usage");
                }
                value => {
                    set_template(&mut template, PathBuf::from(value))?;
                }
            }
        }

        if let Some(action) = config_action {
            if template.is_some() || session != "main" {
                bail!("--config and --config-path cannot be combined with workspace options");
            }
            return Ok(action);
        }

        Ok(ParseOutcome::Run(Self { template, session }))
    }

    pub fn help() -> &'static str {
        "tb2d - a spatial terminal workspace manager

Usage: tb2d [--template <workspace.yaml>] [--session <name>]
       tb2d --config

Options:
  --config          Create/open the user workspace YAML in $VISUAL or $EDITOR
  --config-path     Print the user workspace YAML path
  --template <path>  Start from a workspace template
  --session <name>   Restore and save runtime state (default: main)
  -h, --help         Print help
  -V, --version      Print version

Run tb2d without flags to open the built-in 2r, 1r, 3rc, 2r workspace.
A session remembers its template. Pass --template again to replace it."
    }
}

fn set_template(template: &mut Option<PathBuf>, path: PathBuf) -> Result<()> {
    if template.replace(path).is_some() {
        bail!("template path was provided more than once");
    }
    Ok(())
}

fn set_config_action(action: &mut Option<ParseOutcome>, value: ParseOutcome) -> Result<()> {
    if action.replace(value).is_some() {
        bail!("config action was provided more than once");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_template_and_session_options() {
        assert_eq!(
            Cli::parse_from(["--template", "workspace.yaml", "--session=demo"]).unwrap(),
            ParseOutcome::Run(Cli {
                template: Some(PathBuf::from("workspace.yaml")),
                session: "demo".into(),
            })
        );
    }

    #[test]
    fn retains_positional_template_compatibility() {
        assert_eq!(
            Cli::parse_from(["workspace.yaml", "--session", "demo"]).unwrap(),
            ParseOutcome::Run(Cli {
                template: Some(PathBuf::from("workspace.yaml")),
                session: "demo".into(),
            })
        );
    }

    #[test]
    fn defaults_to_session_backed_template_selection() {
        assert_eq!(
            Cli::parse_from([] as [&str; 0]).unwrap(),
            ParseOutcome::Run(Cli { template: None, session: "main".into() })
        );
    }

    #[test]
    fn parses_config_actions() {
        assert_eq!(Cli::parse_from(["--config"]).unwrap(), ParseOutcome::EditConfig);
        assert_eq!(
            Cli::parse_from(["--config-path"]).unwrap(),
            ParseOutcome::PrintConfigPath
        );
        assert!(Cli::parse_from(["--config", "--session", "demo"]).is_err());
    }

    #[test]
    fn rejects_unknown_or_incomplete_options() {
        assert!(Cli::parse_from(["--wat"]).is_err());
        assert!(Cli::parse_from(["--template"]).is_err());
        assert!(Cli::parse_from(["--session"]).is_err());
        assert!(Cli::parse_from(["one.yaml", "two.yaml"]).is_err());
    }
}
