use crate::error::{ApiError, ApiResult};
use sqlx::SqlitePool;

use super::{
    model::{Profile, ProfileFilters, SimplifiedProfile},
    query::{bind_simplified, build_profile_where_clause, execute_count, validate_sort},
};

pub async fn find_by_name(pool: &SqlitePool, name: &str) -> ApiResult<Option<Profile>> {
    sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> ApiResult<Option<Profile>> {
    sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}

pub async fn insert(pool: &SqlitePool, profile: &Profile) -> ApiResult<()> {
    sqlx::query(
        "INSERT INTO profiles (id, name, gender, gender_probability, sample_size, age, age_group, country_id, country_name, country_probability, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&profile.id)
    .bind(&profile.name)
    .bind(&profile.gender)
    .bind(profile.gender_probability)
    .bind(profile.sample_size)
    .bind(profile.age)
    .bind(&profile.age_group)
    .bind(&profile.country_id)
    .bind(&profile.country_name)
    .bind(profile.country_probability)
    .bind(profile.created_at)
    .execute(pool)
    .await
    .map_err(|_| ApiError::internal("Failed to save profile"))?;

    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> ApiResult<bool> {
    let result = sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))?;

    Ok(result.rows_affected() > 0)
}

pub async fn list(
    pool: &SqlitePool,
    filters: &ProfileFilters,
    page: i32,
    limit: i32,
) -> ApiResult<(Vec<SimplifiedProfile>, i64)> {
    let offset = (page - 1) * limit;
    let (sort_by, order) = validate_sort(filters)?;
    let (where_sql, bindings) = build_profile_where_clause(filters);

    let base = "SELECT id, name, gender, gender_probability, age, age_group, country_id, country_name, created_at FROM profiles";
    let count_base = "SELECT COUNT(*) FROM profiles";
    let data_sql = format!(
        "{}{} ORDER BY {} {} LIMIT {} OFFSET {}",
        base, where_sql, sort_by, order, limit, offset
    );
    let count_sql = format!("{}{}", count_base, where_sql);

    let total = execute_count(pool, &count_sql, &bindings).await;
    let mut q = sqlx::query_as::<_, SimplifiedProfile>(&data_sql);
    for b in &bindings {
        q = bind_simplified(q, b);
    }

    let profiles = q
        .fetch_all(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))?;

    Ok((profiles, total))
}

pub async fn export(pool: &SqlitePool, filters: &ProfileFilters) -> ApiResult<Vec<Profile>> {
    let (sort_by, order) = validate_sort(filters)?;
    let (where_sql, bindings) = build_profile_where_clause(filters);
    let sql = format!(
        "SELECT * FROM profiles{} ORDER BY {} {}",
        where_sql, sort_by, order
    );

    let mut q = sqlx::query_as::<_, Profile>(&sql);
    for b in &bindings {
        q = match b {
            super::query::BindValue::Text(s) => q.bind(s.as_str()),
            super::query::BindValue::Int(i) => q.bind(*i),
            super::query::BindValue::Float(f) => q.bind(*f),
        };
    }

    q.fetch_all(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}
