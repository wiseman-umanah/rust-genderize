use axum::{
    extract::{Query, State},
    Json,
};

use crate::{
    api::v1::profiles::push_filter_links,
    error::{ApiError, ApiResult},
    profiles::{
        model::{PaginatedResponse, ProfileFilters, SearchQuery, SimplifiedProfile},
        natural_language::parse_natural_language_query,
        query, repository,
    },
    state::AppState,
};

pub async fn search_profiles(
    State(state): State<AppState>,
    Query(search_query): Query<SearchQuery>,
) -> ApiResult<Json<PaginatedResponse<SimplifiedProfile>>> {
    if search_query.q.trim().is_empty() {
        return Err(ApiError::bad_request("Missing or empty query parameter"));
    }

    let mapping = state.country_mapping.read().await;
    let parsed = parse_natural_language_query(&search_query.q, &mapping)
        .map_err(|_| ApiError::bad_request("Unable to interpret query"))?;
    drop(mapping);

    let page = search_query.page.unwrap_or(1).max(1);
    let limit = search_query.limit.unwrap_or(10).clamp(1, 50);
    let filters = ProfileFilters {
        gender: parsed.gender,
        age_group: parsed.age_group,
        country_id: parsed.country_id,
        min_age: parsed.min_age,
        max_age: parsed.max_age,
        min_gender_probability: None,
        min_country_probability: None,
        sort_by: None,
        order: None,
        page: Some(page),
        limit: Some(limit),
    };

    let (profiles, total) = repository::list(&state.pool, &filters, page, limit).await?;
    let mut link_query = vec![("q", search_query.q)];
    push_filter_links(&filters, &mut link_query);
    let (total_pages, links) =
        query::pagination(page, limit, total, "/api/profiles/search", &link_query);

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
