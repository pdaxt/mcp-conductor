mod config;
mod middleware;
mod pool;
mod proxy;
mod routes;

use std::sync::Arc;
use std::time::Duration;

use axum::{
    middleware as axum_mw,
    routing::{get, post},
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use config::GatewayConfig;
use pool::BackendPool;
use proxy::ProxyServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,mcp_gateway=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load config
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "mcpgw.toml".into());

    let config = GatewayConfig::load(&config_path)?;
    tracing::info!(
        backends = config.backends.len(),
        webhooks = config.webhooks.len(),
        "config loaded from {config_path}"
    );

    let host = config.server.host.clone();
    let port = config.server.port;
    let timeout = config.server.timeout_secs;

    // Create backend pool and connect
    let pool = Arc::new(BackendPool::new(config.backends));
    pool.connect_all().await;

    // ── MCP Proxy (Streamable HTTP) ──────────────────────────────────────
    // Claude Code connects here: { "type": "http", "url": "http://localhost:8080/mcp" }
    let ct = tokio_util::sync::CancellationToken::new();
    let pool_for_mcp = pool.clone();

    let mcp_service = StreamableHttpService::new(
        move || Ok(ProxyServer::new(pool_for_mcp.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    // ── REST API ─────────────────────────────────────────────────────────
    let api_routes = Router::new()
        .route("/tools", get(routes::list_tools))
        .route("/tools/call", post(routes::call_tool))
        .route("/backends/{backend}/tools", get(routes::list_backend_tools))
        .route(
            "/backends/{backend}/tools/{tool}",
            post(routes::call_backend_tool),
        )
        .route(
            "/backends/{backend}/reconnect",
            post(routes::reconnect_backend),
        )
        .route("/webhooks/{hook_name}", post(routes::webhook));

    let app = Router::new()
        // MCP Streamable HTTP endpoint — Claude Code connects here
        .nest_service("/mcp", mcp_service)
        // REST API — apps/scripts connect here
        .route("/health", get(routes::health))
        .nest("/api", api_routes)
        .layer(axum_mw::from_fn(middleware::log_middleware))
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(timeout),
        ))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(pool);

    // Start server
    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("mcpgw listening on http://{addr}");
    tracing::info!("");
    tracing::info!("  MCP endpoint (Claude Code connects here):");
    tracing::info!("    http://{addr}/mcp");
    tracing::info!("");
    tracing::info!("  REST API:");
    tracing::info!("    GET  /health");
    tracing::info!("    GET  /api/tools");
    tracing::info!("    POST /api/tools/call");
    tracing::info!("    POST /api/backends/{{name}}/tools/{{tool}}");
    tracing::info!("    POST /api/webhooks/{{hook_name}}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            ct.cancel();
        })
        .await?;

    Ok(())
}
