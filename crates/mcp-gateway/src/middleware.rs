use axum::{extract::Request, middleware::Next, response::Response};

/// Request logging middleware — logs method, path, status, duration.
pub async fn log_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let start = std::time::Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        method = %method,
        path = %path,
        status = %status.as_u16(),
        duration_ms = %duration.as_millis(),
    );

    response
}
