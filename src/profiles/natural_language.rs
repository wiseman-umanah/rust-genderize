use regex::Regex;
use sqlx::SqlitePool;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct CountryEntry {
    pub country_name: String,
    pub demonym: String,
}

#[derive(Debug, Default)]
pub struct ParsedFilters {
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub country_id: Option<String>,
    pub min_age: Option<i32>,
    pub max_age: Option<i32>,
}

pub async fn build_country_mapping(
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
                .unwrap_or_else(|| format!("{}an", country_name));
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

pub fn parse_natural_language_query(
    query: &str,
    mapping: &HashMap<String, CountryEntry>,
) -> Result<ParsedFilters, ()> {
    let mut filters = ParsedFilters::default();
    let q = query.to_lowercase();

    if q.contains("female") || q.contains("females") {
        filters.gender = Some("female".to_string());
    } else if q.contains("male") || q.contains("males") {
        filters.gender = Some("male".to_string());
    }

    if q.contains("child") || q.contains("children") {
        filters.age_group = Some("child".to_string());
    } else if q.contains("teenager") || q.contains("teen") {
        filters.age_group = Some("teenager".to_string());
    } else if q.contains("senior") || q.contains("elderly") {
        filters.age_group = Some("senior".to_string());
    } else if q.contains("adult") || q.contains("adults") {
        filters.age_group = Some("adult".to_string());
    }

    if q.contains("young") {
        filters.min_age = Some(16);
        filters.max_age = Some(24);
    }

    if let Some(age) = extract_age(&q, r"(?:above|over|older than)\s+(\d+)") {
        filters.min_age = Some(age);
    }
    if let Some(age) = extract_age(&q, r"(?:below|under|younger than)\s+(\d+)") {
        filters.max_age = Some(age);
    }
    if let Some((min, max)) = extract_age_range(&q) {
        filters.min_age = Some(min);
        filters.max_age = Some(max);
    }

    'outer: for (country_id, entry) in mapping.iter() {
        let name_lower = entry.country_name.to_lowercase();
        let demonym_lower = entry.demonym.to_lowercase();
        let id_lower = country_id.to_lowercase();

        for token in [&name_lower, &demonym_lower, &id_lower] {
            if let Ok(regex) = Regex::new(&format!(r"\b{}\b", regex::escape(token))) {
                if regex.is_match(&q) {
                    filters.country_id = Some(country_id.clone());
                    break 'outer;
                }
            }
        }
    }

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
    Regex::new(pattern)
        .ok()?
        .captures(query)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
}

fn extract_age_range(query: &str) -> Option<(i32, i32)> {
    let caps = Regex::new(r"(\d+)\s*(?:to|-|and)\s*(\d+)")
        .ok()?
        .captures(query)?;
    let min = caps.get(1)?.as_str().parse().ok()?;
    let max = caps.get(2)?.as_str().parse().ok()?;
    Some((min, max))
}
