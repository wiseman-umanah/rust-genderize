use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use axum_extra::extract::OptionalQuery;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;


#[tokio::main]
async fn main() {
    let cors = CorsLayer::new().allow_origin(Any);

    let pool = SqlitePool::connect("sqlite://./data.db").await.expect("Failed to connect to database");
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    
    let app = Router::new()
        .route("/", get(health))
        .route("/api/classify", get(process_data))
        .route("/api/profiles", post(create_profile).get(get_profiles))
        .route("/api/profiles/{id}", get(get_profile).delete(delete_profile))
        .layer(cors)
        .with_state(pool);

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

async fn create_profile(
    State(pool): State<SqlitePool>,
    Json(body): Json<CreateProfileRequest>,
) -> impl IntoResponse {

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "Missing or empty name parameter"
            })),
        );
    }

    // Check if profile already exists
    if let Ok(Some(existing_profile)) = sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE name = ?")
        .bind(&name)
        .fetch_optional(&pool)
        .await
    {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "message": "Profile already exists",
                "data": existing_profile
            })),
        );
    }
	
	
    // Get genderize data
    let genderize = match fetch_genderize_data(&name).await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e
                })),
            );
        }
    };
    
    // Validate genderize response
    if genderize.gender.is_none() || genderize.count.unwrap_or(0) == 0 {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Genderize returned an invalid response"
            })),
        );
    }

    // Get agify data
    let agify = match fetch_agify_data(&name).await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e
                })),
            );
        }
    };
    
    // Validate agify response
    if agify.age.is_none() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Agify returned an invalid response"
            })),
        );
    }


    // Get nationality data
    let country = match fetch_nationalize_data(&name).await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e
                })),
            );
        }
    };
    
    // Validate nationalize response
    if country.country_id.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Nationalize returned an invalid response"
            })),
        );
    }
    let profile = Profile {
        id: Uuid::now_v7().to_string(),
        name: name.clone(),
        gender: genderize.gender.unwrap(),
        gender_probability: genderize.probability.unwrap_or(0.0),
        sample_size: genderize.count.unwrap_or(0) as i64,
        age: agify.age.unwrap(),
        age_group: determine_age_group(agify.age.unwrap()),
        country_id: country.country_id,
        country_probability: country.country_probability,
        created_at: Utc::now(),
    };

    // Save to database
    if let Err(_) = sqlx::query(
        "INSERT INTO profiles (id, name, gender, gender_probability, sample_size, age, age_group, country_id, country_probability, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&profile.id)
        .bind(&profile.name)
        .bind(&profile.gender)
        .bind(profile.gender_probability)
        .bind(profile.sample_size)
        .bind(profile.age)
        .bind(&profile.age_group)
        .bind(&profile.country_id)
        .bind(profile.country_probability)
        .bind(profile.created_at)
        .execute(&pool)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": "Failed to save profile"
            })),
        );
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "success",
            "data": profile
        })),
    )

}


// Request and Response structures
#[derive(Deserialize)]
struct CreateProfileRequest {
    name: String,
}

#[derive(Serialize, FromRow, Debug)]
struct Profile {
    id: String,
    name: String,
    gender: String,
    gender_probability: f64,
    sample_size: i64,
    age: i32,
    age_group: String,
    country_id: String,
    country_probability: f64,
    created_at: DateTime<Utc>,
}

#[derive(Serialize, FromRow, Debug)]
struct SimplifiedProfile {
    id: String,
    name: String,
    gender: String,
    age: i32,
    age_group: String,
    country_id: String,
}

#[derive(Deserialize)]
struct GenderizeResponse {
    gender: Option<String>,
    probability: Option<f64>,
    count: Option<u64>,
}

#[derive(Deserialize)]
struct AgifyResponse {
    age: Option<i32>,
}

#[derive(Deserialize)]
struct NationalizeResponse {
    country: Vec<Country>,
}

#[derive(Deserialize)]
struct Country {
    country_id: String,
    probability: f64,
}

#[derive(Deserialize)]
struct ProcessedNationalizeResponse {
    country_id: String,
    country_probability: f64,
}

// Query parameters for filtering
#[derive(Deserialize)]
struct ProfileFilters {
    gender: Option<String>,
    country_id: Option<String>,
    age_group: Option<String>,
}

// The incoming query parameter
#[derive(Deserialize)]
struct QueryParams {
    name: String,
}

// Helper functions
fn determine_age_group(age: i32) -> String {
    match age {
        0..=12 => "child".to_string(),
        13..=19 => "teenager".to_string(),
        20..=59 => "adult".to_string(),
        _ => "senior".to_string(),
    }
}

// API fetching functions
async fn fetch_genderize_data(name: &str) -> Result<GenderizeResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.genderize.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Genderize API request failed")?;

    response
        .json::<GenderizeResponse>()
        .await
        .map_err(|e| format!("Failed to parse Genderize response: {}", e))
}

async fn fetch_agify_data(name: &str) -> Result<AgifyResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.agify.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Agify API request failed")?;

    response
        .json::<AgifyResponse>()
        .await
        .map_err(|e| format!("Failed to parse Agify response: {}", e))
}

async fn fetch_nationalize_data(name: &str) -> Result<ProcessedNationalizeResponse, String> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.nationalize.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Nationalize API request failed")?;

    let data: NationalizeResponse = response
        .json()
        .await
        .map_err(|_| "Failed to parse Nationalize response")?;

    if data.country.is_empty() {
        return Ok(ProcessedNationalizeResponse {
            country_id: String::new(),
            country_probability: 0.0,
        });
    }

    let max_country = data
        .country
        .iter()
        .max_by(|a, b| a.probability.partial_cmp(&b.probability).unwrap())
        .unwrap();

    let selected_country = max_country.country_id.clone();

    Ok(ProcessedNationalizeResponse {
        country_id: selected_country,
        country_probability: max_country.probability,
    })
}

// Additional endpoints
async fn get_profile(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE id = ?")
        .bind(&id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "data": profile
            })),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "status": "error",
                "message": "Profile not found"
            })),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": "Database error"
            })),
        ),
    }
}

async fn get_profiles(
    State(pool): State<SqlitePool>,
    Query(filters): Query<ProfileFilters>,
) -> impl IntoResponse {
    let mut query = "SELECT id, name, gender, age, age_group, country_id FROM profiles".to_string();
    let mut where_clauses = Vec::new();

    if filters.gender.is_some() || filters.country_id.is_some() || filters.age_group.is_some() {
        query.push_str(" WHERE ");
        
        if let Some(gender) = &filters.gender {
            where_clauses.push(format!("gender = '{}'", gender.to_lowercase()));
        }

        if let Some(country_id) = &filters.country_id {
            where_clauses.push(format!("country_id = '{}'", country_id.to_uppercase()));
        }

        if let Some(age_group) = &filters.age_group {
            where_clauses.push(format!("age_group = '{}'", age_group.to_lowercase()));
        }

        query.push_str(&where_clauses.join(" AND "));
    }

    match sqlx::query_as::<_, SimplifiedProfile>(&query)
        .fetch_all(&pool)
        .await
    {
        Ok(profiles) => {
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "success",
                    "count": profiles.len(),
                    "data": profiles
                })),
            )
        }
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Database error"
                })),
            )
        }
    }
}

async fn delete_profile(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                StatusCode::NO_CONTENT.into_response()
            } else {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "status": "error",
                        "message": "Profile not found"
                    })),
                ).into_response()
            }
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": "Database error"
            })),
        ).into_response(),
    }
}
