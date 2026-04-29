pub mod export;
pub mod profiles;
pub mod search;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::{
    auth,
    middleware::{api_version, rate_limit},
    state::AppState,
};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/classify", get(profiles::classify))
        .route(
            "/profiles",
            post(profiles::create_profile).get(profiles::list_profiles),
        )
        .route("/profiles/export", get(export::export_profiles))
        .route("/profiles/search", get(search::search_profiles))
        .route(
            "/profiles/{id}",
            get(profiles::get_profile).delete(profiles::delete_profile),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ))
        .layer(middleware::from_fn(api_version::require_api_version))
        .layer(middleware::from_fn_with_state(
            state,
            rate_limit::rate_limit,
        ))
}
