mod api;
mod app;
mod auth;
mod config;
mod db;
mod error;
mod middleware;
mod profiles;
mod state;
mod users;

use config::Config;
use state::AppState;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    
    // Initialize JWT crypto provider
    jsonwebtoken::CryptoProvider::install_default().expect("Failed to install crypto provider");
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "insighta_backend=info,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let pool = sqlx::SqlitePool::connect(&config.database_url)
        .await
        .expect("Failed to connect to database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    if let Err(e) = db::seed::seed_database(&pool).await {
        tracing::warn!("Failed to seed database: {e}");
    }

    let demonyms = db::seed::load_demonyms("demonyms.json");
    let country_mapping = Arc::new(RwLock::new(
        profiles::natural_language::build_country_mapping(&pool, &demonyms).await,
    ));

    let state = AppState {
        pool,
        config: config.clone(),
        country_mapping,
        demonyms: Arc::new(demonyms),
        rate_limiter: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = app::router(state);
    let listener = tokio::net::TcpListener::bind((config.host.as_str(), config.port))
        .await
        .expect("Failed to bind TCP listener");

    tracing::info!(
        "Server is running on http://{}:{} (backend_base_url={}, web_base_url={})",
        config.host,
        config.port,
        config.backend_base_url,
        config.web_base_url
    );
    axum::serve(listener, app).await.unwrap();
}
