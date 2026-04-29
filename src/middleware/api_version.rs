use axum::{body::Body, http::Request, middleware::Next, response::Response};

use crate::error::ApiError;

pub async fn require_api_version(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let version = req
        .headers()
        .get("X-API-Version")
        .and_then(|value| value.to_str().ok());

    if version != Some("1") {
        return Err(ApiError::bad_request("API version header required"));
    }

    Ok(next.run(req).await)
}
