mod dal;
mod models;
mod state;
mod tools;
mod utils;

use std::sync::Arc;

use axum::Router;
use dotenvy::dotenv;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

use crate::state::State;

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    dotenv().ok();
    flexi_logger::Logger::try_with_env_or_str("info")?.start()?;

    let state = Arc::new(State::new().await?);

    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true);

    let mcp_service = StreamableHttpService::new(
        {
            let state = Arc::clone(&state);

            move || Ok(crate::tools::Weight::new(Arc::clone(&state)))
        },
        LocalSessionManager::default().into(),
        config,
    );

    let app = Router::new().nest_service("/mcp", mcp_service);
    let listener = TcpListener::bind("0.0.0.0:8080").await?;

    log::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
