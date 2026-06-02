use anyhow::{Context, Result};
use tb2d::{app::App, cli::{Cli, ParseOutcome}, config::Workspace, session::SessionStore, terminal};
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
    };
    let store = SessionStore::new(cli.session);
    let mut restored = match store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(%error, "failed to restore session; using defaults");
            Default::default()
        }
    };
    let template = cli
        .template
        .map(|path| path.canonicalize()
            .with_context(|| format!("failed to resolve template {}", path.display())))
        .transpose()?
        .or_else(|| restored.template.clone());
    let workspace = match &template {
        Some(path) => Workspace::load(path)?,
        None => Workspace::default_template()?,
    };
    restored.template = template.clone();
    let mut app = App::new(workspace, restored)?;
    let result = terminal::run(&mut app);
    store.save(&app.session_state())?;
    result
}
