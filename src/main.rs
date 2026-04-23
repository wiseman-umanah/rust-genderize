use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::OptionalQuery;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

// ─── Shared State ────────────────────────────────────────────────────────────
// CountryMapping: country_id → (country_name, demonym)
// Stored behind Arc<RwLock<...>> so create_profile can refresh it without a restart.
type CountryMapping = Arc<RwLock<HashMap<String, CountryEntry>>>;

#[derive(Clone, Debug)]
struct CountryEntry {
    country_name: String,
    demonym: String,
}

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    country_mapping: CountryMapping,
    // Static demonym lookup loaded once from demonyms.json at startup.
    // This is DATA not code — update demonyms.json, not source.
    demonyms: Arc<HashMap<String, String>>,
}

// ─── Main ─────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    let cors = CorsLayer::new().allow_origin(Any);

    let pool = SqlitePool::connect("sqlite://./data.db")
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    if let Err(e) = seed_database(&pool).await {
        eprintln!("Failed to seed database: {}", e);
    }

    // Load demonyms from file — data, not code
    let demonyms = load_demonyms("demonyms.json");

    // Build the initial country mapping from the profiles table
    let country_mapping = Arc::new(RwLock::new(
        build_country_mapping(&pool, &demonyms).await,
    ));

    let state = AppState {
        pool,
        country_mapping,
        demonyms: Arc::new(demonyms),
    };

    let app = Router::new()
        .route("/", get(health))
        .route("/api/classify", get(process_data))
        .route("/api/profiles", post(create_profile).get(get_profiles))
        .route("/api/profiles/search", get(search_profiles))
        .route("/api/profiles/{id}", get(get_profile).delete(delete_profile))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server is running on port 3000");
    axum::serve(listener, app).await.unwrap();
}

// ─── Health ───────────────────────────────────────────────────────────────────
async fn health() -> &'static str {
    "Backend is running as expected"
}

// ─── Classify ─────────────────────────────────────────────────────────────────
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

// ─── Create Profile ───────────────────────────────────────────────────────────
async fn create_profile(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let name = match body["name"].as_str() {
        Some(n) => n.trim().to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": "Missing or empty name parameter"
                })),
            );
        }
    };

    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "status": "error",
                "message": "Missing or empty name parameter"
            })),
        );
    }

    // Return existing profile if it already exists
    if let Ok(Some(existing)) =
        sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE name = ?")
            .bind(&name)
            .fetch_optional(&state.pool)
            .await
    {
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "message": "Profile already exists",
                "data": existing
            })),
        );
    }

    // Fetch all three APIs
    let genderize = match fetch_genderize_data(&name).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"status": "error", "message": e})),
            )
        }
    };

    if genderize.gender.is_none() || genderize.count.unwrap_or(0) == 0 {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Genderize returned an invalid response"
            })),
        );
    }

    let agify = match fetch_agify_data(&name).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"status": "error", "message": e})),
            )
        }
    };

    if agify.age.is_none() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Agify returned an invalid response"
            })),
        );
    }

    let country = match fetch_nationalize_data(&name).await {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({"status": "error", "message": e})),
            )
        }
    };

    if country.country_id.is_empty() {
        return (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "status": "error",
                "message": "Nationalize returned an invalid response"
            })),
        );
    }

    // Resolve country_name from demonyms file; fall back to country_id if unknown
    let country_name = state
        .demonyms
        .get(&country.country_id)
        .cloned()
        .unwrap_or_else(|| country.country_id.clone());

    let age = agify.age.unwrap();

    let profile = Profile {
        id: Uuid::now_v7().to_string(),
        name: name.clone(),
        gender: genderize.gender.unwrap(),
        gender_probability: genderize.probability.unwrap_or(0.0),
        sample_size: genderize.count.unwrap_or(0) as i64,
        age,
        age_group: determine_age_group(age),
        country_id: country.country_id.clone(),
        country_name: country_name.clone(),
        country_probability: country.country_probability,
        created_at: Utc::now(),
    };

    if let Err(_) = sqlx::query(
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
    .execute(&state.pool)
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

    // Refresh country mapping so the new country is immediately searchable
    let updated_mapping = build_country_mapping(&state.pool, &state.demonyms).await;
    *state.country_mapping.write().await = updated_mapping;

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "success",
            "data": profile
        })),
    )
}

// ─── Get Single Profile ────────────────────────────────────────────────────────
async fn get_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, Profile>("SELECT * FROM profiles WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await
    {
        Ok(Some(profile)) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "success", "data": profile})),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"status": "error", "message": "Profile not found"})),
        ),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": "Database error"})),
        ),
    }
}

// ─── Get Profiles (filtered) ───────────────────────────────────────────────────
async fn get_profiles(
    State(state): State<AppState>,
    Query(filters): Query<ProfileFilters>,
) -> impl IntoResponse {
    let page = filters.page.unwrap_or(1).max(1);
    let limit = filters.limit.unwrap_or(10).min(50).max(1);
    let offset = (page - 1) * limit;

    let sort_by = filters.sort_by.as_deref().unwrap_or("created_at");
    let order = filters.order.as_deref().unwrap_or("desc");

    if !["age", "created_at", "gender_probability"].contains(&sort_by) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "Invalid sort_by parameter"})),
        );
    }
    if !["asc", "desc"].contains(&order) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "Invalid order parameter"})),
        );
    }

    let (where_sql, bindings) = build_profile_where_clause(&filters);

    let base = "SELECT id, name, gender, gender_probability, age, age_group, country_id, country_name, created_at FROM profiles";
    let count_base = "SELECT COUNT(*) FROM profiles";

    let data_sql = format!(
        "{}{} ORDER BY {} {} LIMIT {} OFFSET {}",
        base, where_sql, sort_by, order, limit, offset
    );
    let count_sql = format!("{}{}", count_base, where_sql);

    let total_count = execute_count(&state.pool, &count_sql, &bindings).await;

    let mut q = sqlx::query_as::<_, SimplifiedProfile>(&data_sql);
    for b in &bindings {
        q = bind_value(q, b);
    }

    match q.fetch_all(&state.pool).await {
        Ok(profiles) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "page": page,
                "limit": limit,
                "total": total_count,
                "data": profiles
            })),
        ),
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"status": "error", "message": "Database error"})),
            )
        }
    }
}

// ─── Search Profiles (natural language) ───────────────────────────────────────
async fn search_profiles(
    State(state): State<AppState>,
    Query(search_query): Query<SearchQuery>,
) -> impl IntoResponse {
    if search_query.q.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "Missing or empty query parameter"})),
        );
    }

    // Take a read lock — cheap, non-blocking for concurrent readers
    let mapping = state.country_mapping.read().await;
    let filters = match parse_natural_language_query(&search_query.q, &mapping) {
        Ok(f) => f,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"status": "error", "message": "Unable to interpret query"})),
            )
        }
    };
    drop(mapping); // release lock before hitting DB

    let page = search_query.page.unwrap_or(1).max(1);
    let limit = search_query.limit.unwrap_or(10).min(50).max(1);
    let offset = (page - 1) * limit;

    let profile_filters = ProfileFilters {
        gender: filters.gender,
        age_group: filters.age_group,
        country_id: filters.country_id,
        min_age: filters.min_age,
        max_age: filters.max_age,
        min_gender_probability: None,
        min_country_probability: None,
        sort_by: None,
        order: None,
        page: Some(page),
        limit: Some(limit),
    };

    let (where_sql, bindings) = build_profile_where_clause(&profile_filters);

    let base = "SELECT id, name, gender, gender_probability, age, age_group, country_id, country_name, created_at FROM profiles";
    let count_base = "SELECT COUNT(*) FROM profiles";

    let data_sql = format!(
        "{}{} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        base, where_sql, limit, offset
    );
    let count_sql = format!("{}{}", count_base, where_sql);

    let total_count = execute_count(&state.pool, &count_sql, &bindings).await;

    let mut q = sqlx::query_as::<_, SimplifiedProfile>(&data_sql);
    for b in &bindings {
        q = bind_value(q, b);
    }

    match q.fetch_all(&state.pool).await {
        Ok(profiles) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "page": page,
                "limit": limit,
                "total": total_count,
                "data": profiles
            })),
        ),
        Err(e) => {
            eprintln!("Database error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"status": "error", "message": "Database error"})),
            )
        }
    }
}

// ─── Delete Profile ────────────────────────────────────────────────────────────
async fn delete_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM profiles WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await
    {
        Ok(result) if result.rows_affected() > 0 => StatusCode::NO_CONTENT.into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"status": "error", "message": "Profile not found"})),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": "Database error"})),
        )
            .into_response(),
    }
}

// ─── Query Builder Helpers ─────────────────────────────────────────────────────

// Binding enum so we can collect mixed types without Box<dyn Any> gymnastics
#[derive(Clone)]
enum BindValue {
    Text(String),
    Int(i32),
    Float(f64),
}

fn build_profile_where_clause(filters: &ProfileFilters) -> (String, Vec<BindValue>) {
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

fn bind_value<'q>(
    q: sqlx::query::QueryAs<'q, sqlx::Sqlite, SimplifiedProfile, sqlx::sqlite::SqliteArguments<'q>>,
    val: &'q BindValue,
) -> sqlx::query::QueryAs<'q, sqlx::Sqlite, SimplifiedProfile, sqlx::sqlite::SqliteArguments<'q>> {
    match val {
        BindValue::Text(s) => q.bind(s.as_str()),
        BindValue::Int(i) => q.bind(*i),
        BindValue::Float(f) => q.bind(*f),
    }
}

async fn execute_count(pool: &SqlitePool, sql: &str, bindings: &[BindValue]) -> i64 {
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

// ─── Country Mapping ───────────────────────────────────────────────────────────

// Loads demonyms.json into a HashMap<country_id, demonym_string>.
// This replaces create_demonym entirely. Add/fix entries in the JSON file, not in code.
fn load_demonyms(path: &str) -> HashMap<String, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            eprintln!("Failed to parse demonyms.json: {}", e);
            HashMap::new()
        }),
        Err(e) => {
            eprintln!("Failed to read demonyms.json: {}", e);
            HashMap::new()
        }
    }
}

// Builds the live country mapping from the profiles table.
// Keyed by country_id for O(1) lookup.
// Called at startup AND after every create_profile so it never goes stale.
async fn build_country_mapping(
    pool: &SqlitePool,
    demonyms: &HashMap<String, String>,
) -> HashMap<String, CountryEntry> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT country_id, country_name FROM profiles WHERE country_name IS NOT NULL AND country_name != ''",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|(country_id, country_name)| {
            let demonym = demonyms
                .get(&country_id)
                .cloned()
                .unwrap_or_else(|| format!("{}an", country_name)); // last-resort fallback
            (
                country_id.clone(),
                CountryEntry {
                    country_name,
                    demonym,
                },
            )
        })
        .collect()
}

// ─── Natural Language Parser ───────────────────────────────────────────────────
fn parse_natural_language_query(
    query: &str,
    mapping: &HashMap<String, CountryEntry>,
) -> Result<ParsedFilters, ()> {
    let mut filters = ParsedFilters::default();
    let q = query.to_lowercase();

    // Gender
    if q.contains("female") || q.contains("females") {
        filters.gender = Some("female".to_string());
    } else if q.contains("male") || q.contains("males") {
        filters.gender = Some("male".to_string());
    }

    // Age group keywords
    if q.contains("child") || q.contains("children") {
        filters.age_group = Some("child".to_string());
    } else if q.contains("teenager") || q.contains("teen") {
        filters.age_group = Some("teenager".to_string());
    } else if q.contains("senior") || q.contains("elderly") {
        filters.age_group = Some("senior".to_string());
    } else if q.contains("adult") || q.contains("adults") {
        filters.age_group = Some("adult".to_string());
    }

    // "young" shorthand
    if q.contains("young") {
        filters.min_age = Some(16);
        filters.max_age = Some(24);
    }

    // above/over/older than N
    if let Some(age) = extract_age(&q, r"(?:above|over|older than)\s+(\d+)") {
        filters.min_age = Some(age);
    }

    // below/under/younger than N
    if let Some(age) = extract_age(&q, r"(?:below|under|younger than)\s+(\d+)") {
        filters.max_age = Some(age);
    }

    // N to/- M range
    if let Some((min, max)) = extract_age_range(&q) {
        filters.min_age = Some(min);
        filters.max_age = Some(max);
    }

    // Country matching: check country_name and demonym from the live mapping
    'outer: for (country_id, entry) in mapping.iter() {
        let name_lower = entry.country_name.to_lowercase();
        let demonym_lower = entry.demonym.to_lowercase();
        let id_lower = country_id.to_lowercase();

        for token in [&name_lower, &demonym_lower, &id_lower] {
            if q.contains(token.as_str()) {
                filters.country_id = Some(country_id.clone());
                break 'outer;
            }
        }
    }

    // Reject if nothing was parsed
    if filters.gender.is_none()
        && filters.age_group.is_none()
        && filters.country_id.is_none()
        && filters.min_age.is_none()
        && filters.max_age.is_none()
    {
        return Err(());
    }

    Ok(filters)
}

fn extract_age(query: &str, pattern: &str) -> Option<i32> {
    regex::Regex::new(pattern)
        .ok()?
        .captures(query)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
}

fn extract_age_range(query: &str) -> Option<(i32, i32)> {
    let caps = regex::Regex::new(r"(\d+)\s*(?:to|-|and)\s*(\d+)")
        .ok()?
        .captures(query)?;
    let min = caps.get(1)?.as_str().parse().ok()?;
    let max = caps.get(2)?.as_str().parse().ok()?;
    Some((min, max))
}

// ─── API Fetch Helpers ─────────────────────────────────────────────────────────
async fn fetch_genderize_data(name: &str) -> Result<GenderizeResponse, String> {
    reqwest::Client::new()
        .get("https://api.genderize.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Genderize API request failed".to_string())?
        .json::<GenderizeResponse>()
        .await
        .map_err(|e| format!("Failed to parse Genderize response: {}", e))
}

async fn fetch_agify_data(name: &str) -> Result<AgifyResponse, String> {
    reqwest::Client::new()
        .get("https://api.agify.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Agify API request failed".to_string())?
        .json::<AgifyResponse>()
        .await
        .map_err(|e| format!("Failed to parse Agify response: {}", e))
}

async fn fetch_nationalize_data(name: &str) -> Result<ProcessedNationalizeResponse, String> {
    let data: NationalizeResponse = reqwest::Client::new()
        .get("https://api.nationalize.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Nationalize API request failed".to_string())?
        .json()
        .await
        .map_err(|_| "Failed to parse Nationalize response".to_string())?;

    if data.country.is_empty() {
        return Ok(ProcessedNationalizeResponse {
            country_id: String::new(),
            country_probability: 0.0,
        });
    }

    let best = data
        .country
        .iter()
        .max_by(|a, b| a.probability.partial_cmp(&b.probability).unwrap())
        .unwrap();

    Ok(ProcessedNationalizeResponse {
        country_id: best.country_id.clone(),
        country_probability: best.probability,
    })
}

// ─── Seed ─────────────────────────────────────────────────────────────────────
async fn seed_database(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let seed_file = std::fs::read_to_string("seed_profiles.json")?;
    let seed_data: SeedData = serde_json::from_str(&seed_file)?;

    println!("Seeding database with {} profiles...", seed_data.profiles.len());

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM profiles").execute(&mut *tx).await?;

    for (index, p) in seed_data.profiles.iter().enumerate() {
        sqlx::query(
            "INSERT INTO profiles (id, name, gender, gender_probability, sample_size, age, age_group, country_id, country_name, country_probability, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(&p.name)
        .bind(&p.gender)
        .bind(p.gender_probability)
        .bind(0_i64)
        .bind(p.age)
        .bind(&p.age_group)
        .bind(&p.country_id)
        .bind(&p.country_name)
        .bind(p.country_probability)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;

        if (index + 1) % 100 == 0 {
            println!("Seeded {} profiles...", index + 1);
        }
    }

    tx.commit().await?;
    println!("Database seeding completed successfully!");
    Ok(())
}

// ─── Helpers ──────────────────────────────────────────────────────────────────
fn determine_age_group(age: i32) -> String {
    match age {
        0..=12 => "child".to_string(),
        13..=19 => "teenager".to_string(),
        20..=59 => "adult".to_string(),
        _ => "senior".to_string(),
    }
}

// ─── Types ────────────────────────────────────────────────────────────────────
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
    country_name: String,
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
    country_name: String,
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

#[derive(Deserialize)]
struct ProfileFilters {
    gender: Option<String>,
    age_group: Option<String>,
    country_id: Option<String>,
    min_age: Option<i32>,
    max_age: Option<i32>,
    min_gender_probability: Option<f64>,
    min_country_probability: Option<f64>,
    sort_by: Option<String>,
    order: Option<String>,
    page: Option<i32>,
    limit: Option<i32>,
}

#[derive(Deserialize)]
struct QueryParams {
    name: String,
}

#[derive(Deserialize)]
struct SeedData {
    profiles: Vec<SeedProfile>,
}

#[derive(Deserialize)]
struct SeedProfile {
    name: String,
    gender: String,
    gender_probability: f64,
    age: i32,
    age_group: String,
    country_id: String,
    country_name: String,
    country_probability: f64,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    page: Option<i32>,
    limit: Option<i32>,
}

#[derive(Debug, Default)]
struct ParsedFilters {
    gender: Option<String>,
    age_group: Option<String>,
    country_id: Option<String>,
    min_age: Option<i32>,
    max_age: Option<i32>,
}