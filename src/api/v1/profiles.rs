use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use axum_extra::extract::OptionalQuery;
use chrono::Utc;

use crate::{
    auth::middleware::{require_admin, AuthenticatedUser},
    error::{ApiError, ApiResult},
    profiles::{
        external::GenderizeResponse,
        model::{ClassifyQuery, CreateProfileRequest, PaginatedResponse, ProfileFilters},
        query, repository, service,
    },
    state::AppState,
};

pub async fn classify(
    OptionalQuery(params): OptionalQuery<ClassifyQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let query = params
        .ok_or_else(|| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "name is not a string"))?;
    let name = query.name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("Missing or empty name parameter"));
    }

    let response = reqwest::Client::new()
        .get("https://api.genderize.io")
        .query(&[("name", &name)])
        .send()
        .await
        .map_err(|_| ApiError::bad_gateway("Upstream or server failure"))?;

    let genderize: GenderizeResponse = response
        .json()
        .await
        .map_err(|_| ApiError::internal("Upstream or server failure"))?;

    let gender = genderize.gender.ok_or_else(|| {
        ApiError::new(
            StatusCode::OK,
            "No prediction available for the provided name",
        )
    })?;
    let sample_size = genderize.count.unwrap_or(0);
    if sample_size == 0 {
        return Err(ApiError::new(
            StatusCode::OK,
            "No prediction available for the provided name",
        ));
    }

    let probability = genderize.probability.unwrap_or(0.0);
    Ok(Json(serde_json::json!({
        "status": "success",
        "data": {
            "name": name,
            "gender": gender,
            "probability": probability,
            "sample_size": sample_size,
            "is_confident": probability >= 0.7 && sample_size >= 100,
            "processed_at": Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
        }
    })))
}

pub async fn create_profile(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(body): Json<CreateProfileRequest>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    require_admin(&user)?;
    let (created, profile) = service::create_profile(&state, body.name).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    let message = if created {
        None
    } else {
        Some("Profile already exists")
    };

    Ok((
        status,
        Json(serde_json::json!({
            "status": "success",
            "message": message,
            "data": profile
        })),
    ))
}

pub async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let profile = repository::find_by_id(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("Profile not found"))?;

    Ok(Json(
        serde_json::json!({"status": "success", "data": profile}),
    ))
}

pub async fn list_profiles(
    State(state): State<AppState>,
    Query(filters): Query<ProfileFilters>,
) -> ApiResult<Json<PaginatedResponse<crate::profiles::model::SimplifiedProfile>>> {
    let page = filters.page.unwrap_or(1).max(1);
    let limit = filters.limit.unwrap_or(10).clamp(1, 50);
    let (profiles, total) = repository::list(&state.pool, &filters, page, limit).await?;
    let mut link_query = Vec::new();
    push_filter_links(&filters, &mut link_query);
    let (total_pages, links) = query::pagination(page, limit, total, "/api/profiles", &link_query);

    Ok(Json(PaginatedResponse {
        status: "success",
        page,
        limit,
        total,
        total_pages,
        links,
        data: profiles,
    }))
}

pub async fn delete_profile(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    require_admin(&user)?;
    if repository::delete(&state.pool, &id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found("Profile not found"))
    }
}

pub fn push_filter_links(filters: &ProfileFilters, link_query: &mut Vec<(&'static str, String)>) {
    if let Some(value) = &filters.gender {
        link_query.push(("gender", value.clone()));
    }
    if let Some(value) = &filters.age_group {
        link_query.push(("age_group", value.clone()));
    }
    if let Some(value) = &filters.country_id {
        link_query.push(("country_id", value.clone()));
    }
    if let Some(value) = filters.min_age {
        link_query.push(("min_age", value.to_string()));
    }
    if let Some(value) = filters.max_age {
        link_query.push(("max_age", value.to_string()));
    }
    if let Some(value) = filters.min_gender_probability {
        link_query.push(("min_gender_probability", value.to_string()));
    }
    if let Some(value) = filters.min_country_probability {
        link_query.push(("min_country_probability", value.to_string()));
    }
    if let Some(value) = &filters.sort_by {
        link_query.push(("sort_by", value.clone()));
    }
    if let Some(value) = &filters.order {
        link_query.push(("order", value.clone()));
    }
}
