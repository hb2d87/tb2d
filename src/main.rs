use anyhow::Result;
use tb2d::{app::App, cli::Cli, config::Workspace, session::SessionStore, terminal};
use tracing::warn;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let workspace = Workspace::load(&cli.template)?;
    let store = SessionStore::new(cli.session);
    let restored = match store.load() {
        Ok(state) => state,
        Err(error) => {
            warn!(%error, "failed to restore session; using defaults");
            Default::default()
        }
    };
    let mut app = App::new(workspace, restored)?;
    let result = terminal::run(&mut app);
    store.save(&app.session_state())?;
    result
}
