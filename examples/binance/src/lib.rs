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
pub struct BinanceResource {
    pub symbol: String,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct BinancePrice {
    pub symbol: String,
    pub price: String,
}

const BINANCE_API_URL: &str = "https://api.binance.com/api/v3/ticker/price";

#[oracle_component]
async fn oracle_request(settings: Settings) -> Result<Payload> {
    let mut resources: HashMap<String, BinanceResource> = HashMap::new();
    let mut symbols = Vec::new();

    // Compose one query to Binance API for all data feeds
    for feed in settings.data_feeds.iter() {
        let data: BinanceResource = serde_json::from_str(&feed.data)?;
        resources.insert(feed.id.clone(), data.clone());
        if !symbols.contains(&data.symbol) {
            symbols.push(data.symbol);
        }
    }

    // Build URL with proper parameters
    let symbols_json = serde_json::to_string(&symbols)?;
    let url = format!(
        "{}?symbols={}",
        BINANCE_API_URL,
        symbols_json
    );
    let url = Url::parse(&url)?;
    
    println!("Requesting URL: {}", url);  // Debug log

    let mut req = Request::builder();
    req.method(Method::Get);
    req.uri(url);
    req.header("accept", "application/json");

    let req = req.build();
    let resp: Response = send(req).await?;
    
    println!("Response status: {}", resp.status());  // Debug log
    
    if *resp.status() != 200 {
        let body = String::from_utf8_lossy(resp.body());
        anyhow::bail!("Failed to fetch data from Binance API: status {}, body: {}", resp.status(), body);
    }

    let prices: Vec<BinancePrice> = serde_json::from_slice(resp.body())?;
    let mut values = Vec::new();

    // Create a HashMap for quick price lookups
    let price_map: HashMap<String, String> = prices
        .into_iter()
        .map(|p| (p.symbol, p.price))
        .collect();

    for (feed_id, resource) in resources.iter() {
        if let Some(price_str) = price_map.get(&resource.symbol) {
            if let Ok(price) = price_str.parse::<f64>() {
                values.push(DataFeedResult {
                    id: feed_id.clone(),
                    value: DataFeedResultValue::Numerical(price),
                });
            }
        }
    }

    Ok(Payload { values })
} 