use roseflower::config::Config;
use roseflower::db::init_db;
use roseflower::server::{build_router, AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,roseflower=debug")),
        )
        .init();

    info!("Starting Roseflower - Dedicated Real-time Multiplayer Service...");

    let config = Config::load("config.toml")?;
    let db_path = config.resolve_db_path();
    let db_pool = init_db(&db_path).await?;

    let state = AppState {
        db: db_pool,
        config: Arc::new(config.clone()),
    };

    let app = build_router(state);
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = TcpListener::bind(&bind_addr).await?;

    info!("Roseflower Service listening on http://{}", bind_addr);
    info!("Serving live multiplayer tracker on http://{}/ and http://{}/multi", bind_addr, bind_addr);

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
