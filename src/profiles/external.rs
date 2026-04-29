use serde::Deserialize;

#[derive(Deserialize)]
pub struct GenderizeResponse {
    pub gender: Option<String>,
    pub probability: Option<f64>,
    pub count: Option<u64>,
}

#[derive(Deserialize)]
pub struct AgifyResponse {
    pub age: Option<i32>,
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

pub struct ProcessedNationalizeResponse {
    pub country_id: String,
    pub country_probability: f64,
}

pub async fn fetch_genderize_data(name: &str) -> Result<GenderizeResponse, String> {
    reqwest::Client::new()
        .get("https://api.genderize.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Genderize API request failed".to_string())?
        .json::<GenderizeResponse>()
        .await
        .map_err(|e| format!("Failed to parse Genderize response: {e}"))
}

pub async fn fetch_agify_data(name: &str) -> Result<AgifyResponse, String> {
    reqwest::Client::new()
        .get("https://api.agify.io")
        .query(&[("name", name)])
        .send()
        .await
        .map_err(|_| "Agify API request failed".to_string())?
        .json::<AgifyResponse>()
        .await
        .map_err(|e| format!("Failed to parse Agify response: {e}"))
}

pub async fn fetch_nationalize_data(name: &str) -> Result<ProcessedNationalizeResponse, String> {
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
