use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::Utc;

use crate::{
    error::{ApiError, ApiResult},
    profiles::{
        model::{ExportQuery, ProfileFilters},
        repository,
    },
    state::AppState,
};

pub async fn export_profiles(
    State(state): State<AppState>,
    Query(query): Query<ExportQuery>,
) -> ApiResult<impl IntoResponse> {
    if query.format.as_deref().unwrap_or("csv") != "csv" {
        return Err(ApiError::bad_request("Only csv export is supported"));
    }

    let filters: ProfileFilters = query.into();
    let profiles = repository::export(&state.pool, &filters).await?;
    let mut writer = csv::Writer::from_writer(vec![]);

    writer
        .write_record([
            "id",
            "name",
            "gender",
            "gender_probability",
            "age",
            "age_group",
            "country_id",
            "country_name",
            "country_probability",
            "created_at",
        ])
        .map_err(|_| ApiError::internal("Failed to write CSV"))?;

    for profile in profiles {
        writer
            .write_record([
                profile.id,
                profile.name,
                profile.gender,
                profile.gender_probability.to_string(),
                profile.age.to_string(),
                profile.age_group,
                profile.country_id,
                profile.country_name,
                profile.country_probability.to_string(),
                profile.created_at.to_rfc3339(),
            ])
            .map_err(|_| ApiError::internal("Failed to write CSV"))?;
    }

    let body = String::from_utf8(
        writer
            .into_inner()
            .map_err(|_| ApiError::internal("Failed to finish CSV"))?,
    )
    .map_err(|_| ApiError::internal("Failed to encode CSV"))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/csv".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"profiles_{}.csv\"",
            Utc::now().format("%Y%m%d%H%M%S")
        )
        .parse()
        .unwrap(),
    );

    Ok((StatusCode::OK, headers, body))
}
