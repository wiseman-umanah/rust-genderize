use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    users::model::{NewGithubUser, User},
};

pub async fn find_by_github_id(pool: &SqlitePool, github_id: &str) -> ApiResult<Option<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE github_id = ?")
        .bind(github_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> ApiResult<Option<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}

pub async fn find_active_by_id(pool: &SqlitePool, id: &str) -> ApiResult<Option<User>> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ? AND is_active = 1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| ApiError::internal("Database error"))
}

pub async fn upsert_from_github(pool: &SqlitePool, input: NewGithubUser) -> ApiResult<User> {
    if let Some(user) = find_by_github_id(pool, &input.github_id).await? {
        let now = Utc::now();
        sqlx::query(
            "UPDATE users
             SET username = ?, email = ?, avatar_url = ?, last_login_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&input.username)
        .bind(&input.email)
        .bind(&input.avatar_url)
        .bind(now)
        .bind(now)
        .bind(&user.id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::internal("Failed to update user"))?;

        return find_by_id(pool, &user.id)
            .await?
            .ok_or_else(|| ApiError::internal("Failed to reload user"));
    }

    let now = Utc::now();
    let id = Uuid::now_v7().to_string();

    sqlx::query(
        "INSERT INTO users (id, github_id, username, email, avatar_url, role, is_active, last_login_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, 'analyst', 1, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.github_id)
    .bind(&input.username)
    .bind(&input.email)
    .bind(&input.avatar_url)
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| ApiError::internal("Failed to create user"))?;

    find_by_id(pool, &id)
        .await?
        .ok_or_else(|| ApiError::internal("Failed to reload user"))
}
