use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Clone, Debug, Serialize, FromRow)]
pub struct User {
    pub id: String,
    pub github_id: String,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
    pub is_active: bool,
    pub last_login_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct NewGithubUser {
    pub github_id: String,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    Admin,
    Analyst,
}

impl Role {
    pub fn from_str(value: &str) -> Self {
        match value {
            "admin" => Self::Admin,
            _ => Self::Analyst,
        }
    }

    pub fn can_write_profiles(&self) -> bool {
        matches!(self, Self::Admin)
    }
}
