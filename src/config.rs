#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_url: String,
    pub jwt_secret: String,
    pub backend_base_url: String,
    pub web_base_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env_or("DATABASE_URL", "sqlite://./data.db"),
            host: env_or("HOST", "0.0.0.0"),
            port: env_or("PORT", "3000").parse().unwrap_or(3000),
            github_client_id: env_or("GITHUB_CLIENT_ID", ""),
            github_client_secret: env_or("GITHUB_CLIENT_SECRET", ""),
            github_redirect_url: env_or(
                "GITHUB_REDIRECT_URL",
                "http://localhost:3000/auth/github/callback",
            ),
            jwt_secret: env_or("JWT_SECRET", "dev-secret-change-me"),
            backend_base_url: env_or("BACKEND_BASE_URL", "http://localhost:3000"),
            web_base_url: env_or("WEB_BASE_URL", "http://localhost:5173"),
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
