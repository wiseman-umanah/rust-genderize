use axum::{
    body::Body,
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use serde::Serialize;

use crate::{
    auth::tokens,
    error::ApiError,
    state::AppState,
    users::{model::Role, repository},
};

#[derive(Clone, Debug, Serialize)]
pub struct AuthenticatedUser {
    pub user: crate::users::model::User,
}

impl AuthenticatedUser {
    pub fn role(&self) -> Role {
        Role::from_str(&self.user.role)
    }
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let token = bearer_token(&req)
        .or_else(|| cookie_token(&req))
        .ok_or_else(|| ApiError::unauthorized("Authentication required"))?;

    let claims = tokens::verify_access_token(&state.config, &token)?;
    let user = repository::find_by_id(&state.pool, &claims.sub)
        .await?
        .ok_or_else(|| ApiError::unauthorized("User not found"))?;

    if !user.is_active {
        return Err(ApiError::forbidden("User is inactive"));
    }

    req.extensions_mut().insert(AuthenticatedUser { user });

    Ok(next.run(req).await)
}

pub fn require_admin(user: &AuthenticatedUser) -> Result<(), ApiError> {
    if user.role().can_write_profiles() {
        Ok(())
    } else {
        Err(ApiError::forbidden("Admin role required"))
    }
}

fn bearer_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(ToOwned::to_owned)
}

fn cookie_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "insighta_access").then(|| value.to_string())
            })
        })
}
