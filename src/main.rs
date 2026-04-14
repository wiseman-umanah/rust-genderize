use axum::{http::StatusCode, routing::get, Json, Router};
use axum_extra::extract::OptionalQuery;
use chrono::Utc;
use reqwest;
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new().allow_origin(Any);

    let app = Router::new()
        .route("/", get(health))
        .route("/api/classify", get(process_data))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server is running on port 3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "Backend is running as expected"
}

#[axum::debug_handler]
async fn process_data(
    OptionalQuery(params): OptionalQuery<QueryParams>,
) -> (StatusCode, Json<serde_json::Value>) {
    let query = match params {
        Some(q) => q,
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "name is not a string"
                })),
            )
        }
    };

    let name = query.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "Missing or empty name parameter"
            })),
        );
    }

    let client = reqwest::Client::new();
    let api_result = client
        .get("https://api.genderize.io")
        .query(&[("name", &name)])
        .send()
        .await;

    let api_response = match api_result {
        Ok(res) => res,
        Err(_) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                "status": "error",
                "message": "Upstream or server failure"
                })),
            )
        }
    };

    let genderize: GenderizeResponse = match api_response.json().await {
        Ok(data) => data,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Upstream or server failure"
                })),
            )
        }
    };

    let gender = match genderize.gender {
        Some(g) => g,
        None => {
            return (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "No prediction available for the provided name"
                })),
            )
        }
    };

    let sample_size = genderize.count.unwrap_or(0);
    if sample_size == 0 {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "error",
                "message": "No prediction available for the provided name"
            })),
        );
    }

    let probability = genderize.probability.unwrap_or(0.0);
    let is_confident = probability >= 0.7 && sample_size >= 100;
    let processed_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "success",
            "data": {
                "name": name,
                "gender": gender,
                "probability": probability,
                "sample_size": sample_size,
                "is_confident": is_confident,
                "processed_at": processed_at
            }
        })),
    )
}

#[derive(Deserialize)]
struct GenderizeResponse {
    gender: Option<String>,
    probability: Option<f64>,
    count: Option<u64>,
}

// The incoming query parameter
#[derive(Deserialize)]
struct QueryParams {
    name: String,
}
