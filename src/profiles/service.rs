use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    profiles::{external, model::Profile, natural_language, repository},
    state::AppState,
};

pub async fn create_profile(state: &AppState, name: String) -> ApiResult<(bool, Profile)> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ApiError::bad_request("Missing or empty name parameter"));
    }

    if let Some(existing) = repository::find_by_name(&state.pool, &name).await? {
        return Ok((false, existing));
    }

    let genderize = external::fetch_genderize_data(&name)
        .await
        .map_err(ApiError::bad_gateway)?;
    if genderize.gender.is_none() || genderize.count.unwrap_or(0) == 0 {
        return Err(ApiError::bad_gateway(
            "Genderize returned an invalid response",
        ));
    }

    let agify = external::fetch_agify_data(&name)
        .await
        .map_err(ApiError::bad_gateway)?;
    let age = agify
        .age
        .ok_or_else(|| ApiError::bad_gateway("Agify returned an invalid response"))?;

    let country = external::fetch_nationalize_data(&name)
        .await
        .map_err(ApiError::bad_gateway)?;
    if country.country_id.is_empty() {
        return Err(ApiError::bad_gateway(
            "Nationalize returned an invalid response",
        ));
    }

    let country_name = state
        .demonyms
        .get(&country.country_id)
        .cloned()
        .unwrap_or_else(|| country.country_id.clone());

    let profile = Profile {
        id: Uuid::now_v7().to_string(),
        name,
        gender: genderize.gender.unwrap(),
        gender_probability: genderize.probability.unwrap_or(0.0),
        sample_size: genderize.count.unwrap_or(0) as i64,
        age,
        age_group: determine_age_group(age),
        country_id: country.country_id,
        country_name,
        country_probability: country.country_probability,
        created_at: Utc::now(),
    };

    repository::insert(&state.pool, &profile).await?;
    let updated_mapping =
        natural_language::build_country_mapping(&state.pool, &state.demonyms).await;
    *state.country_mapping.write().await = updated_mapping;

    Ok((true, profile))
}

pub fn determine_age_group(age: i32) -> String {
    match age {
        0..=12 => "child".to_string(),
        13..=19 => "teenager".to_string(),
        20..=59 => "adult".to_string(),
        _ => "senior".to_string(),
    }
}
