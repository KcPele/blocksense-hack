use anyhow::Result;
use blocksense_sdk::{
    oracle::{DataFeedResult, DataFeedResultValue, Payload, Settings},
    oracle_component,
    spin::http::{send, Method, Request, Response},
};
use std::collections::HashMap;
use serde::Deserialize;
use url::Url;

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct GeckoResource {
    pub gecko_id: String,
    pub vs_currency: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct GeckoPrice {
    #[serde(flatten)]
    pub prices: HashMap<String, HashMap<String, f64>>,
}

const COINGECKO_API_URL: &str = "https://api.coingecko.com/api/v3/simple/price";

#[oracle_component]
async fn oracle_request(settings: Settings) -> Result<Payload> {
    let mut resources: HashMap<String, GeckoResource> = HashMap::new();
    let mut ids = Vec::new();
    let mut vs_currencies = Vec::new();

    // Compose one query to Coingecko API for all data feeds
    for feed in settings.data_feeds.iter() {
        let data: GeckoResource = serde_json::from_str(&feed.data)?;
        resources.insert(feed.id.clone(), data.clone());
        if !ids.contains(&data.gecko_id) {
            ids.push(data.gecko_id);
        }
        if !vs_currencies.contains(&data.vs_currency.to_lowercase()) {
            vs_currencies.push(data.vs_currency.to_lowercase());
        }
    }

    // Build URL with proper parameters
    let url = format!(
        "{}?ids={}&vs_currencies={}",
        COINGECKO_API_URL,
        ids.join(","),
        vs_currencies.join(",")
    );
    let url = Url::parse(&url)?;

    // Get API key from capabilities
    let api_key = settings.capabilities.iter()
        .find(|cap| cap.id == "GECKO_API_KEY")
        .map(|cap| cap.data.as_str())
        .ok_or_else(|| anyhow::anyhow!("GECKO_API_KEY not found in capabilities"))?;
    
    println!("Requesting URL: {}", url);  // Debug log

    let mut req = Request::builder();
    req.method(Method::Get);
    req.uri(url);
    // For demo API key, we don't need to send the API key header
    req.header("accept", "application/json");

    let req = req.build();
    let resp: Response = send(req).await?;
    
    println!("Response status: {}", resp.status());  // Debug log
    
    if *resp.status() != 200 {
        let body = String::from_utf8_lossy(resp.body());
        anyhow::bail!("Failed to fetch data from Coingecko API: status {}, body: {}", resp.status(), body);
    }

    let prices: GeckoPrice = serde_json::from_slice(resp.body())?;
    let mut values = Vec::new();

    for (feed_id, resource) in resources.iter() {
        if let Some(coin_prices) = prices.prices.get(&resource.gecko_id) {
            if let Some(&price) = coin_prices.get(&resource.vs_currency.to_lowercase()) {
                values.push(DataFeedResult {
                    id: feed_id.clone(),
                    value: DataFeedResultValue::Numerical(price),
                });
            }
        }
    }

    Ok(Payload { values })
} 