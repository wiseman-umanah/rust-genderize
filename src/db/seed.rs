use chrono::Utc;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use uuid::Uuid;

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

pub fn load_demonyms(path: &str) -> HashMap<String, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse demonyms.json: {e}");
            HashMap::new()
        }),
        Err(e) => {
            tracing::warn!("Failed to read demonyms.json: {e}");
            HashMap::new()
        }
    }
}

pub async fn seed_database(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    let seed_file = std::fs::read_to_string("seed_profiles.json")?;
    let seed_data: SeedData = serde_json::from_str(&seed_file)?;
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM profiles")
        .fetch_one(pool)
        .await?;

    if existing > 0 {
        tracing::info!("Skipping seed; profiles table already contains {existing} rows");
        return Ok(());
    }

    tracing::info!(
        "Seeding database with {} profiles",
        seed_data.profiles.len()
    );
    let mut tx = pool.begin().await?;

    for p in seed_data.profiles.iter() {
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
    }

    tx.commit().await?;
    Ok(())
}
