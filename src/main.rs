use anyhow::{Context, Result};
use serde_json::json;
use std::{backtrace::Backtrace, path::Path, process::Command};
use tb2d::{
    app::App,
    cli::{Cli, ParseOutcome},
    config::Workspace,
    session::SessionStore,
    terminal,
};
use tracing::warn;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = match Cli::parse()? {
        ParseOutcome::Run(cli) => cli,
        ParseOutcome::Help => {
            println!("{}", Cli::help());
            return Ok(());
        }
        ParseOutcome::Version => {
            println!("tb2d {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        ParseOutcome::PrintConfigPath => {
            println!("{}", Workspace::user_default_template_path().display());
            return Ok(());
        }
        ParseOutcome::EditConfig => {
            let path = Workspace::ensure_user_default_template()?;
            open_editor(&path)?;
            return Ok(());
        }
    };
    let session_name = cli.session.clone();
    let store = SessionStore::new(cli.session);
    install_panic_logger(store.clone());
    let _ = store.append_diagnostic(
        "session-opened",
        &[
            ("session", json!(session_name)),
            ("session_path", json!(store.path().display().to_string())),
            ("diagnostics_path", json!(store.diagnostics_path().display().to_string())),
        ],
    );
    let mut restored = match store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(%error, "failed to restore session; using defaults");
            let _ = store.append_diagnostic(
                "session-restore-failed",
                &[("error", json!(format!("{error:#}")))],
            );
            Default::default()
        }
    };
    let explicit_template = cli.template.is_some();
    let default_template = if !explicit_template && restored.template.is_none() {
        match Workspace::ensure_user_default_template() {
            Ok(path) => Some(path),
            Err(error) => {
                warn!(%error, "failed to seed user default config; using built-in default");
                let _ = store.append_diagnostic(
                    "config-seed-failed",
                    &[("error", json!(format!("{error:#}")))],
                );
                None
            }
        }
    } else {
        None
    };
    let template = cli
        .template
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("failed to resolve template {}", path.display()))
        })
        .transpose()?
        .or_else(|| restored.template.clone())
        .or(default_template);
    let runtime_workspace = (!explicit_template).then(|| restored.workspace.take()).flatten();
    let workspace = match runtime_workspace {
        Some(workspace) => match workspace.validate() {
            Ok(()) => workspace,
            Err(error) => {
                warn!(%error, "restored runtime workspace is invalid; using template/default");
                let _ = store.append_diagnostic(
                    "runtime-workspace-invalid",
                    &[("error", json!(format!("{error:#}")))],
                );
                load_workspace(&template, &store)?
            }
        },
        None => load_workspace(&template, &store)?,
    };
    let _ = store.append_diagnostic(
        "workspace-loaded",
        &[
            ("workspace", json!(workspace.name.clone())),
            ("columns", json!(workspace.columns.len())),
            ("template", json!(template.as_ref().map(|path| path.display().to_string()))),
        ],
    );
    restored.template = template.clone();
    if explicit_template {
        restored.workspace = None;
    }
    let mut app = App::new(workspace, restored)?;
    let result = terminal::run(&mut app, &store);
    if let Err(error) = &result {
        let _ = store.append_diagnostic(
            "terminal-run-failed",
            &[("error", json!(format!("{error:#}")))],
        );
    }
    if let Err(error) = store.save(&app.session_state()) {
        let _ = store.append_diagnostic(
            "session-save-failed",
            &[("error", json!(format!("{error:#}")))],
        );
        return Err(error);
    }
    let _ = store.append_diagnostic(
        "session-closed",
        &[("status", json!(if result.is_ok() { "ok" } else { "terminal-error" }))],
    );
    result
}

fn open_editor(path: &Path) -> Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_owned());
    let status = Command::new("sh")
        .arg("-lc")
        .arg(format!("{editor} \"$1\""))
        .arg("tb2d-editor")
        .arg(path)
        .status()
        .with_context(|| format!("failed to open editor for {}", path.display()))?;
    if !status.success() {
        anyhow::bail!("editor exited with status {status}");
    }
    Ok(())
}

fn load_workspace(template: &Option<std::path::PathBuf>, store: &SessionStore) -> Result<Workspace> {
    match template {
        Some(path) => match Workspace::load(path) {
            Ok(workspace) => Ok(workspace),
            Err(error) => {
                let _ = store.append_diagnostic(
                    "workspace-load-failed",
                    &[
                        ("template", json!(path.display().to_string())),
                        ("error", json!(format!("{error:#}"))),
                    ],
                );
                Err(error)
            }
        },
        None => Workspace::default_template(),
    }
}

fn install_panic_logger(store: SessionStore) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .map(str::to_owned)
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic payload".to_owned());
        let location = info
            .location()
            .map(|location| {
                format!(
                    "{}:{}:{}",
                    location.file(),
                    location.line(),
                    location.column()
                )
            });
        let backtrace = Backtrace::force_capture().to_string();
        let _ = store.append_diagnostic(
            "panic",
            &[
                ("payload", json!(payload)),
                ("location", json!(location)),
                ("backtrace", json!(backtrace)),
            ],
        );
        previous(info);
    }));
}
