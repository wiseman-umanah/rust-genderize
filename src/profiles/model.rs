use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Serialize, FromRow, Debug, Clone)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub gender: String,
    pub gender_probability: f64,
    pub sample_size: i64,
    pub age: i32,
    pub age_group: String,
    pub country_id: String,
    pub country_name: String,
    pub country_probability: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, Debug, Clone)]
pub struct SimplifiedProfile {
    pub id: String,
    pub name: String,
    pub gender: String,
    pub age: i32,
    pub age_group: String,
    pub country_id: String,
    pub country_name: String,
    pub gender_probability: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateProfileRequest {
    pub name: String,
}

#[derive(Deserialize, Clone, Default)]
pub struct ProfileFilters {
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub country_id: Option<String>,
    pub min_age: Option<i32>,
    pub max_age: Option<i32>,
    pub min_gender_probability: Option<f64>,
    pub min_country_probability: Option<f64>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub page: Option<i32>,
    pub limit: Option<i32>,
}

#[derive(Deserialize)]
pub struct ClassifyQuery {
    pub name: String,
}

#[derive(Serialize)]
pub struct PaginationLinks {
    #[serde(rename = "self")]
    pub self_link: String,
    pub next: Option<String>,
    pub prev: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub status: &'static str,
    pub page: i32,
    pub limit: i32,
    pub total: i64,
    pub total_pages: i64,
    pub links: PaginationLinks,
    pub data: Vec<T>,
}

#[derive(Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub country_id: Option<String>,
    pub min_age: Option<i32>,
    pub max_age: Option<i32>,
    pub min_gender_probability: Option<f64>,
    pub min_country_probability: Option<f64>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
}

impl From<ExportQuery> for ProfileFilters {
    fn from(query: ExportQuery) -> Self {
        Self {
            gender: query.gender,
            age_group: query.age_group,
            country_id: query.country_id,
            min_age: query.min_age,
            max_age: query.max_age,
            min_gender_probability: query.min_gender_probability,
            min_country_probability: query.min_country_probability,
            sort_by: query.sort_by,
            order: query.order,
            page: None,
            limit: None,
        }
    }
}
