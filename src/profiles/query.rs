use crate::{error::ApiError, profiles::model::PaginationLinks};
use sqlx::{query::QueryAs, sqlite::SqliteArguments, Sqlite, SqlitePool};

use super::model::{ProfileFilters, SimplifiedProfile};

#[derive(Clone)]
pub enum BindValue {
    Text(String),
    Int(i32),
    Float(f64),
}

pub fn validate_sort(filters: &ProfileFilters) -> Result<(&str, &str), ApiError> {
    let sort_by = filters.sort_by.as_deref().unwrap_or("created_at");
    let order = filters.order.as_deref().unwrap_or("desc");

    if !["age", "created_at", "gender_probability"].contains(&sort_by) {
        return Err(ApiError::bad_request("Invalid sort_by parameter"));
    }
    if !["asc", "desc"].contains(&order) {
        return Err(ApiError::bad_request("Invalid order parameter"));
    }

    Ok((sort_by, order))
}

pub fn build_profile_where_clause(filters: &ProfileFilters) -> (String, Vec<BindValue>) {
    let mut clauses: Vec<&str> = Vec::new();
    let mut bindings: Vec<BindValue> = Vec::new();

    if let Some(g) = &filters.gender {
        clauses.push("gender = ?");
        bindings.push(BindValue::Text(g.to_lowercase()));
    }
    if let Some(ag) = &filters.age_group {
        clauses.push("age_group = ?");
        bindings.push(BindValue::Text(ag.to_lowercase()));
    }
    if let Some(c) = &filters.country_id {
        clauses.push("country_id = ?");
        bindings.push(BindValue::Text(c.to_uppercase()));
    }
    if let Some(v) = filters.min_age {
        clauses.push("age >= ?");
        bindings.push(BindValue::Int(v));
    }
    if let Some(v) = filters.max_age {
        clauses.push("age <= ?");
        bindings.push(BindValue::Int(v));
    }
    if let Some(v) = filters.min_gender_probability {
        clauses.push("gender_probability >= ?");
        bindings.push(BindValue::Float(v));
    }
    if let Some(v) = filters.min_country_probability {
        clauses.push("country_probability >= ?");
        bindings.push(BindValue::Float(v));
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    (where_sql, bindings)
}

pub fn bind_simplified<'q>(
    q: QueryAs<'q, Sqlite, SimplifiedProfile, SqliteArguments<'q>>,
    val: &'q BindValue,
) -> QueryAs<'q, Sqlite, SimplifiedProfile, SqliteArguments<'q>> {
    match val {
        BindValue::Text(s) => q.bind(s.as_str()),
        BindValue::Int(i) => q.bind(*i),
        BindValue::Float(f) => q.bind(*f),
    }
}

pub async fn execute_count(pool: &SqlitePool, sql: &str, bindings: &[BindValue]) -> i64 {
    let mut q = sqlx::query_scalar::<_, i64>(sql);
    for b in bindings {
        q = match b {
            BindValue::Text(s) => q.bind(s.as_str()),
            BindValue::Int(i) => q.bind(*i),
            BindValue::Float(f) => q.bind(*f),
        };
    }
    q.fetch_one(pool).await.unwrap_or(0)
}

pub fn pagination(
    page: i32,
    limit: i32,
    total: i64,
    base_path: &str,
    query: &[(&str, String)],
) -> (i64, PaginationLinks) {
    let total_pages = if total == 0 {
        0
    } else {
        ((total as f64) / (limit as f64)).ceil() as i64
    };

    let build = |target_page: i32| {
        let mut parts = vec![format!("page={target_page}"), format!("limit={limit}")];
        for (key, value) in query {
            if !value.is_empty() {
                parts.push(format!("{key}={}", urlencoding::encode(value)));
            }
        }
        format!("{base_path}?{}", parts.join("&"))
    };

    let links = PaginationLinks {
        self_link: build(page),
        next: if (page as i64) < total_pages {
            Some(build(page + 1))
        } else {
            None
        },
        prev: if page > 1 {
            Some(build(page - 1))
        } else {
            None
        },
    };

    (total_pages, links)
}
