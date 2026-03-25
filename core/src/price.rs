use serde::{Deserialize, Serialize};
use std::time::Duration;

const COINGECKO_URL: &str = "https://api.coingecko.com/api/v3/simple/price?ids=kaspa&vs_currencies=usd,btc&include_market_cap=true&include_24hr_vol=true&include_24hr_change=true";

#[derive(Debug, Deserialize, Serialize)]
pub struct PriceResponse {
    pub kaspa: KaspaPrice,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct KaspaPrice {
    pub usd: f64,
    pub btc: f64,
    pub usd_market_cap: f64,
    pub usd_24h_vol: f64,
    pub usd_24h_change: f64,
}

pub fn default_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("tui4kas/1.0")
        .build()
        .unwrap()
}

pub async fn fetch_market_data(
    client: Option<&reqwest::Client>,
) -> Result<KaspaPrice, reqwest::Error> {
    let owned;
    let client = match client {
        Some(c) => c,
        None => {
            owned = default_client();
            &owned
        }
    };

    let resp: PriceResponse = client
        .get(COINGECKO_URL)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp.kaspa)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_coingecko_response() {
        let json = r#"{
            "kaspa": {
                "usd": 0.15,
                "btc": 0.0000025,
                "usd_market_cap": 3800000000.0,
                "usd_24h_vol": 50000000.0,
                "usd_24h_change": 5.5
            }
        }"#;
        let resp: PriceResponse = serde_json::from_str(json).unwrap();
        assert!((resp.kaspa.usd - 0.15).abs() < f64::EPSILON);
        assert!((resp.kaspa.btc - 0.0000025).abs() < f64::EPSILON);
        assert!((resp.kaspa.usd_market_cap - 3_800_000_000.0).abs() < 1.0);
        assert!((resp.kaspa.usd_24h_vol - 50_000_000.0).abs() < 1.0);
        assert!((resp.kaspa.usd_24h_change - 5.5).abs() < f64::EPSILON);
    }
}
