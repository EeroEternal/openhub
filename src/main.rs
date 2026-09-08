use gitcell::server::AppState as GitcellState;
use openhub::mail::Mailer;
use openhub::server::HubState;
use openhub::{config::Config, error::Result, server, store};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::default();
    let gitcell = GitcellState::new(
        &config.data_dir,
        &config.cells_dir,
        &config.cells_storage_dir,
        config.cells_lease_ttl_secs,
    )?;
    let db = store::connect(&config.database_url).await?;
    store::migrate(&db).await?;
    let static_dir = config.static_dir.clone().filter(|p| p.is_dir());
    if let Some(ref dir) = config.static_dir {
        if static_dir.is_none() {
            tracing::warn!(path = %dir.display(), "OPENHUB_STATIC_DIR is set but not a directory");
        } else {
            info!(path = %dir.display(), "serving Admin UI");
        }
    }
    let hub = HubState {
        gitcell,
        db,
        mail: Mailer::from_env(),
        public_origin: config.public_origin.clone(),
    };
    let app = server::create_router_with_static(hub, static_dir);

    let addr = format!("{}:{}", config.host, config.port);
    info!(
        "Starting OpenHub on {} (public origin {})",
        addr, config.public_origin
    );

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind to {}: {}", addr, e))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {}", e))?;

    Ok(())
}
