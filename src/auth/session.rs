use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub status: &'static str,
    pub access_token: String,
    pub refresh_token: String,
}
