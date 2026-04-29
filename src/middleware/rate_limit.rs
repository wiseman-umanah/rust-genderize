use axum::{
    body::Body,
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::{error::ApiError, state::AppState};

pub async fn rate_limit(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let path = req.uri().path().to_string();
    let is_auth = path.starts_with("/auth/");
    let limit = if is_auth { 10 } else { 60 };
    let now = Utc::now().timestamp();
    let key = rate_limit_key(&req, is_auth);

    let mut buckets = state.rate_limiter.lock().await;
    let entries = buckets.entry(key).or_default();
    entries.retain(|timestamp| now - *timestamp < 60);

    if entries.len() >= limit {
        return Err(ApiError::too_many_requests("Too many requests"));
    }

    entries.push(now);
    drop(buckets);

    Ok(next.run(req).await)
}

fn rate_limit_key(req: &Request<Body>, is_auth: bool) -> String {
    if is_auth {
        return "auth:global".to_string();
    }

    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("anonymous");
    let digest = Sha256::digest(auth.as_bytes());
    format!("api:{digest:x}")
}
