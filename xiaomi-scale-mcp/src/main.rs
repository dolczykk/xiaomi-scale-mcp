mod auth;
mod config;
mod dal;
mod models;
mod state;
mod tools;
mod utils;

use std::sync::Arc;

use axum::{Router, middleware};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::{StreamableHttpServerConfig, StreamableHttpService};
use tokio::net::TcpListener;

use crate::auth::require_bearer_token;
use crate::config::Config;
use crate::state::State;

#[tokio::main]
async fn main() -> anyhow::Result<(), anyhow::Error> {
    flexi_logger::Logger::try_with_env_or_str("info")?.start()?;

    let app_config = Config::load()?;
    let state = Arc::new(State::new(&app_config).await?);

    let transport_config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true);

    let mcp_service = StreamableHttpService::new(
        {
            let state = Arc::clone(&state);

            move || Ok(crate::tools::Weight::new(Arc::clone(&state)))
        },
        LocalSessionManager::default().into(),
        transport_config,
    );

    let authorization_token = Arc::new(app_config.server.authorization_token.clone());
    let app =
        Router::new()
            .nest_service("/mcp", mcp_service)
            .layer(middleware::from_fn_with_state(
                authorization_token,
                require_bearer_token,
            ));
    let listener = TcpListener::bind(&app_config.server.bind_address).await?;

    log::info!("Server listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
