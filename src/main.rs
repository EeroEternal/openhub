use gitcell::server::AppState;
use openhub::{config::Config, error::Result, server};
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
    let state = AppState::new(
        &config.data_dir,
        &config.cells_dir,
        &config.cells_storage_dir,
        config.cells_lease_ttl_secs,
    )?;
    let app = server::create_router(state);

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
