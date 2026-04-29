use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use crate::{
    api,
    auth::{csrf, github},
    middleware::{logging, rate_limit},
    state::AppState,
};

pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new().allow_origin(Any);

    let auth_routes = Router::new()
        .route("/github", get(github::start))
        .route("/github/callback", get(github::callback))
        .route("/github/exchange", post(github::exchange_device_flow))
        .route("/refresh", post(github::refresh))
        .route("/logout", post(github::logout))
        .route("/csrf", get(csrf::csrf_token))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit,
        ));

    let protected_auth_routes = Router::new()
        .route("/me", get(github::me))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::middleware::require_auth,
        ));

    Router::new()
        .route("/", get(health))
        .nest("/auth", auth_routes.merge(protected_auth_routes))
        .nest("/api", api::v1::router(state.clone()))
        .layer(middleware::from_fn(logging::log_request))
        .layer(cors)
        .with_state(state)
}

async fn health() -> &'static str {
    "Backend is running as expected"
}
