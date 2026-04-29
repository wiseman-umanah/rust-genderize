use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::{
    config::Config,
    error::{ApiError, ApiResult},
    users::model::User,
};

type RefreshTokenRow = (
    String,
    String,
    chrono::DateTime<Utc>,
    Option<chrono::DateTime<Utc>>,
);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

pub fn create_access_token(config: &Config, user: &User) -> ApiResult<String> {
    let now = Utc::now();
    let claims = Claims {
        sub: user.id.clone(),
        username: user.username.clone(),
        role: user.role.clone(),
        iat: now.timestamp() as usize,
        exp: (now + Duration::minutes(3)).timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
    .map_err(|_| ApiError::internal("Failed to create access token"))
}

pub fn verify_access_token(config: &Config, token: &str) -> ApiResult<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| ApiError::unauthorized("Invalid or expired access token"))
}

pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

pub async fn issue_pair(pool: &SqlitePool, config: &Config, user: &User) -> ApiResult<TokenPair> {
    let access_token = create_access_token(config, user)?;
    let refresh_token = format!("rfr_{}_{}", Uuid::now_v7(), Uuid::new_v4());
    let refresh_hash = hash_token(&refresh_token);
    let expires_at = Utc::now() + Duration::minutes(5);

    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(Uuid::now_v7().to_string())
    .bind(&user.id)
    .bind(refresh_hash)
    .bind(expires_at)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|_| ApiError::internal("Failed to store refresh token"))?;

    Ok(TokenPair {
        access_token,
        refresh_token,
    })
}

pub async fn rotate_refresh_token(
    pool: &SqlitePool,
    config: &Config,
    refresh_token: &str,
) -> ApiResult<TokenPair> {
    let token_hash = hash_token(refresh_token);
    let row: Option<RefreshTokenRow> = sqlx::query_as(
        "SELECT id, user_id, expires_at, revoked_at FROM refresh_tokens WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|_| ApiError::internal("Database error"))?;

    let (token_id, user_id, expires_at, revoked_at) =
        row.ok_or_else(|| ApiError::unauthorized("Invalid refresh token"))?;
    if revoked_at.is_some() || expires_at <= Utc::now() {
        return Err(ApiError::unauthorized("Invalid refresh token"));
    }

    sqlx::query("UPDATE refresh_tokens SET revoked_at = ? WHERE id = ?")
        .bind(Utc::now())
        .bind(token_id)
        .execute(pool)
        .await
        .map_err(|_| ApiError::internal("Failed to rotate refresh token"))?;

    let user = crate::users::repository::find_active_by_id(pool, &user_id)
        .await?
        .ok_or_else(|| ApiError::forbidden("User is inactive"))?;

    issue_pair(pool, config, &user).await
}

pub async fn revoke_refresh_token(pool: &SqlitePool, refresh_token: &str) -> ApiResult<()> {
    let token_hash = hash_token(refresh_token);
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = ? WHERE token_hash = ? AND revoked_at IS NULL",
    )
    .bind(Utc::now())
    .bind(token_hash)
    .execute(pool)
    .await
    .map_err(|_| ApiError::internal("Failed to logout"))?;
    Ok(())
}
