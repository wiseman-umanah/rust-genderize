use axum::{http::StatusCode, Json};

pub async fn csrf_token() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "success",
            "csrf_token": "development-csrf-token"
        })),
    )
}
