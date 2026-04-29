use crate::config::Config;
use crate::profiles::natural_language::CountryEntry;
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

pub type CountryMapping = Arc<RwLock<HashMap<String, CountryEntry>>>;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Config,
    pub country_mapping: CountryMapping,
    pub demonyms: Arc<HashMap<String, String>>,
    pub rate_limiter: Arc<Mutex<HashMap<String, Vec<i64>>>>,
}
