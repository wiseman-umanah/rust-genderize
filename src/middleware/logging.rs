use axum::{body::Body, http::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn log_request(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status();
    tracing::info!(
        method = %method,
        endpoint = %path,
        status_code = status.as_u16(),
        response_time_ms = start.elapsed().as_millis(),
        "request completed"
    );
    response
}
