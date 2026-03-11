use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::pool::BackendPool;

pub type AppState = Arc<BackendPool>;

// ── Health ───────────────────────────────────────────────────────────────────

pub async fn health(State(pool): State<AppState>) -> Json<HealthResponse> {
    let connected = pool.connected_backends();
    let configured = pool.configured_backends();
    Json(HealthResponse {
        status: if connected.len() == configured.len() {
            "healthy"
        } else {
            "degraded"
        },
        backends_connected: connected.len(),
        backends_configured: configured.len(),
        connected,
    })
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub backends_connected: usize,
    pub backends_configured: usize,
    pub connected: Vec<String>,
}

// ── List Tools ───────────────────────────────────────────────────────────────

pub async fn list_tools(State(pool): State<AppState>) -> Json<serde_json::Value> {
    let tools = pool.list_all_tools().await;
    Json(serde_json::json!({
        "tools": tools,
        "count": tools.len(),
    }))
}

/// List tools for a specific backend.
pub async fn list_backend_tools(
    State(pool): State<AppState>,
    Path(backend): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let tools = pool.list_tools(&backend).await?;
    Ok(Json(serde_json::json!({
        "backend": backend,
        "tools": tools,
        "count": tools.len(),
    })))
}

// ── Call Tool ────────────────────────────────────────────────────────────────

/// Call a tool on any backend (auto-routed by tool name).
pub async fn call_tool(
    State(pool): State<AppState>,
    Json(req): Json<CallToolRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = pool.call_tool_any(&req.tool, req.arguments).await?;

    Ok(Json(serde_json::json!({
        "tool": req.tool,
        "is_error": result.is_error.unwrap_or(false),
        "content": format!("{:?}", result.content),
    })))
}

/// Call a tool on a specific backend.
pub async fn call_backend_tool(
    State(pool): State<AppState>,
    Path((backend, tool)): Path<(String, String)>,
    Json(body): Json<Option<serde_json::Map<String, serde_json::Value>>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let result = pool.call_tool(&backend, &tool, body).await?;

    Ok(Json(serde_json::json!({
        "backend": backend,
        "tool": tool,
        "is_error": result.is_error.unwrap_or(false),
        "content": format!("{:?}", result.content),
    })))
}

// ── Webhook Receiver ─────────────────────────────────────────────────────────

/// Generic webhook endpoint — receives POST, maps to tool call via config.
pub async fn webhook(
    State(pool): State<AppState>,
    Path(hook_name): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    // For now, pass the whole body as tool arguments.
    // The event bus (phase 3) will handle proper mapping/chaining.
    let arguments = body.as_object().cloned();

    // Convention: webhook name IS the tool name unless overridden by config
    let result = pool.call_tool_any(&hook_name, arguments).await?;

    Ok(Json(serde_json::json!({
        "webhook": hook_name,
        "is_error": result.is_error.unwrap_or(false),
        "content": format!("{:?}", result.content),
    })))
}

// ── Reconnect ────────────────────────────────────────────────────────────────

pub async fn reconnect_backend(
    State(pool): State<AppState>,
    Path(backend): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    pool.reconnect(&backend).await?;
    Ok(Json(serde_json::json!({
        "status": "reconnected",
        "backend": backend,
    })))
}

// ── Request/Response Types ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallToolRequest {
    pub tool: String,
    pub arguments: Option<serde_json::Map<String, serde_json::Value>>,
}

// ── Error Handling ───────────────────────────────────────────────────────────

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let msg = self.0.to_string();
        tracing::error!(error = %msg);

        let status = if msg.contains("not connected") || msg.contains("not found") {
            StatusCode::NOT_FOUND
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };

        (
            status,
            Json(serde_json::json!({
                "error": msg,
            })),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}
