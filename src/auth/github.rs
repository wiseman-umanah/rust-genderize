use axum::{
    extract::{Extension, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        session::{AuthResponse, LogoutRequest, RefreshRequest},
        tokens,
    },
    error::{ApiError, ApiResult},
    state::AppState,
    users::{model::NewGithubUser, repository},
};

#[derive(Deserialize)]
pub struct GithubStartQuery {
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Deserialize)]
pub struct GithubCallbackQuery {
    pub code: String,
    pub state: Option<String>,
    pub code_verifier: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Deserialize)]
pub struct GithubTokenExchangeRequest {
    pub github_token: String,
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GithubUserResponse {
    id: u64,
    login: String,
    email: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Serialize)]
pub struct StatusResponse<'a> {
    status: &'a str,
    message: &'a str,
}

pub async fn start(
    State(state): State<AppState>,
    Query(query): Query<GithubStartQuery>,
) -> ApiResult<Redirect> {
    if state.config.github_client_id.is_empty() {
        return Err(ApiError::internal("GITHUB_CLIENT_ID is not configured"));
    }

    let state_value = query
        .state
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
    let redirect_uri = query
        .redirect_uri
        .unwrap_or_else(|| state.config.github_redirect_url.clone());

    let mut url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user%20user:email&state={}",
        urlencoding::encode(&state.config.github_client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&state_value)
    );

    if let Some(challenge) = query.code_challenge {
        url.push_str("&code_challenge=");
        url.push_str(&urlencoding::encode(&challenge));
        url.push_str("&code_challenge_method=S256");
    }

    Ok(Redirect::temporary(&url))
}

pub async fn callback(
    State(state): State<AppState>,
    Query(query): Query<GithubCallbackQuery>,
) -> ApiResult<impl IntoResponse> {
    if query.state.as_deref().unwrap_or_default().is_empty() {
        return Err(ApiError::bad_request("OAuth state is required"));
    }

    let github_access_token = exchange_code(&state, &query).await?;
    let github_user = fetch_github_user(&github_access_token).await?;
    let user = repository::upsert_from_github(
        &state.pool,
        NewGithubUser {
            github_id: github_user.id.to_string(),
            username: github_user.login,
            email: github_user.email,
            avatar_url: github_user.avatar_url,
        },
    )
    .await?;

    if !user.is_active {
        return Err(ApiError::forbidden("User is inactive"));
    }

    let pair = tokens::issue_pair(&state.pool, &state.config, &user).await?;

    // Check if this is a web portal callback by checking for web redirect URI
    let _is_web_callback = query
        .redirect_uri
        .as_ref()
        .map(|uri| uri.contains("localhost:5173") || uri.contains("127.0.0.1:5173"))
        .unwrap_or(false);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        format!(
            "insighta_access={}; HttpOnly; SameSite=Lax; Path=/; Max-Age=1800",
            pair.access_token
        )
        .parse()
        .unwrap(),
    );

    // Check if this is a web portal callback
    let is_web_callback = query
        .redirect_uri
        .as_ref()
        .map(|uri| uri.contains("localhost:5173") || uri.contains("127.0.0.1:5173"))
        .unwrap_or(false);

    if is_web_callback {
        // For web portal, add redirect header
        let redirect_url = query
            .redirect_uri
            .as_ref()
            .map(|uri| {
                if uri.contains("?") {
                    format!("{}&auth=success", uri)
                } else {
                    format!("{}?auth=success", uri)
                }
            })
            .unwrap_or_else(|| format!("{}?auth=success", state.config.web_base_url));

        headers.insert(header::LOCATION, redirect_url.parse().unwrap());
    }

    let response = AuthResponse {
        status: "success",
        access_token: pair.access_token,
        refresh_token: pair.refresh_token,
    };
    Ok((headers, Json(response)))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let pair =
        tokens::rotate_refresh_token(&state.pool, &state.config, &body.refresh_token).await?;
    Ok(Json(AuthResponse {
        status: "success",
        access_token: pair.access_token,
        refresh_token: pair.refresh_token,
    }))
}

pub async fn me(
    State(_state): State<AppState>,
    Extension(user): Extension<crate::auth::middleware::AuthenticatedUser>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "status": "success",
        "data": {
            "id": user.user.id,
            "github_username": user.user.username,
            "email": user.user.email,
            "avatar_url": user.user.avatar_url,
            "is_active": user.user.is_active,
            "created_at": user.user.created_at,
            "updated_at": user.user.updated_at
        }
    })))
}

pub async fn exchange_device_flow(
    State(state): State<AppState>,
    Json(body): Json<GithubTokenExchangeRequest>,
) -> ApiResult<Json<AuthResponse>> {
    // Fetch GitHub user using the device flow token
    let github_user = fetch_github_user(&body.github_token).await?;
    let user = repository::upsert_from_github(
        &state.pool,
        NewGithubUser {
            github_id: github_user.id.to_string(),
            username: github_user.login,
            email: github_user.email,
            avatar_url: github_user.avatar_url,
        },
    )
    .await?;

    if !user.is_active {
        return Err(ApiError::forbidden("User is inactive"));
    }

    let pair = tokens::issue_pair(&state.pool, &state.config, &user).await?;
    Ok(Json(AuthResponse {
        status: "success",
        access_token: pair.access_token,
        refresh_token: pair.refresh_token,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Json(body): Json<LogoutRequest>,
) -> ApiResult<(StatusCode, Json<StatusResponse<'static>>)> {
    tokens::revoke_refresh_token(&state.pool, &body.refresh_token).await?;
    Ok((
        StatusCode::OK,
        Json(StatusResponse {
            status: "success",
            message: "Logged out",
        }),
    ))
}

async fn exchange_code(state: &AppState, query: &GithubCallbackQuery) -> ApiResult<String> {
    if state.config.github_client_id.is_empty() || state.config.github_client_secret.is_empty() {
        return Err(ApiError::internal("GitHub OAuth is not configured"));
    }

    let mut params = vec![
        ("client_id", state.config.github_client_id.clone()),
        ("client_secret", state.config.github_client_secret.clone()),
        ("code", query.code.clone()),
        ("redirect_uri", state.config.github_redirect_url.clone()),
    ];
    if let Some(code_verifier) = &query.code_verifier {
        params.push(("code_verifier", code_verifier.clone()));
    }

    let token = reqwest::Client::new()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|_| ApiError::bad_gateway("GitHub token exchange failed"))?
        .json::<GithubTokenResponse>()
        .await
        .map_err(|_| ApiError::bad_gateway("Invalid GitHub token response"))?;

    Ok(token.access_token)
}

async fn fetch_github_user(access_token: &str) -> ApiResult<GithubUserResponse> {
    reqwest::Client::new()
        .get("https://api.github.com/user")
        .bearer_auth(access_token)
        .header("User-Agent", "insighta-labs")
        .send()
        .await
        .map_err(|_| ApiError::bad_gateway("GitHub user request failed"))?
        .json::<GithubUserResponse>()
        .await
        .map_err(|_| ApiError::bad_gateway("Invalid GitHub user response"))
}
