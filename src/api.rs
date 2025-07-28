#![allow(dead_code)]

// was vibing while 'coding' this one too, iykwim.
use reqwest;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::error::Error;

// --- Helper functions for deserialization ---
fn deserialize_string_to_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("nan") || s == "." {
        Ok(0.0) // Return 0.0 for empty/None/NaN/'.' strings
    } else {
        s.parse::<f64>().map_err(serde::de::Error::custom)
    }
}

// Your Ohlcv related structs (assuming they work, not directly related to this options issue)
#[derive(Debug, Deserialize, Clone)]
pub struct Ohlcv {
    #[serde(rename = "4. close", deserialize_with = "deserialize_string_to_f64")]
    pub close: f64,
    pub date: String,
}

#[derive(Debug, Deserialize)]
pub struct RawDailyData {
    #[serde(rename = "1. open", deserialize_with = "deserialize_string_to_f64")]
    pub open: f64,
    #[serde(rename = "2. high", deserialize_with = "deserialize_string_to_f64")]
    pub high: f64,
    #[serde(rename = "3. low", deserialize_with = "deserialize_string_to_f64")]
    pub low: f64,
    #[serde(rename = "4. close", deserialize_with = "deserialize_string_to_f64")]
    pub close: f64,
    #[serde(rename = "5. volume", deserialize_with = "deserialize_string_to_f64")]
    pub volume: f64,
}

#[derive(Debug, Deserialize)]
pub struct RawOhlcvResponse {
    #[serde(rename = "Meta Data")]
    pub meta_data: HashMap<String, String>,
    #[serde(rename = "Time Series (Daily)")]
    pub time_series_daily: HashMap<String, RawDailyData>,
}

pub fn transform_raw_data_to_ohlcv_vec(
    raw_time_series_map: HashMap<String, RawDailyData>,
) -> Result<Vec<Ohlcv>, Box<dyn Error>> {
    let mut ohlcv_points: Vec<Ohlcv> = Vec::new();
    let mut sorted_dates: Vec<&String> = raw_time_series_map.keys().collect();
    sorted_dates.sort_unstable();
    for date_str in sorted_dates {
        if let Some(raw_daily_data) = raw_time_series_map.get(date_str) {
            let ohlcv_point = Ohlcv {
                date: date_str.clone(),
                close: raw_daily_data.close,
            };
            ohlcv_points.push(ohlcv_point);
        }
    }
    Ok(ohlcv_points)
}

// --- Options related structs ---
#[derive(Debug, Clone)]
pub enum OptionType {
    Put,
    Call,
}

// --- NEW STRUCT TO MATCH THE TOP-LEVEL JSON ---
#[derive(Debug, Deserialize)]
pub struct AlphaVantageOptionsRawResponse {
    pub endpoint: String,
    pub message: String,
    pub data: Vec<RawHistoricalOptionEntry>, // This now matches "data"
}

// Updated RawHistoricalOptionEntry to include all fields from your example
#[derive(Debug, Deserialize)]
pub struct RawHistoricalOptionEntry {
    #[serde(rename = "contractID")]
    pub contract_id: String, // "contractID" in JSON
    pub symbol: String,
    pub expiration: String,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub strike: f64,
    #[serde(rename = "type")] // "type" is a keyword in Rust, so rename it
    pub option_type_str: String, // Will parse this into OptionType enum later
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub last: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub mark: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub bid: f64,
    #[serde(rename = "bid_size", deserialize_with = "deserialize_string_to_f64")]
    pub bid_size: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub ask: f64,
    #[serde(rename = "ask_size", deserialize_with = "deserialize_string_to_f64")]
    pub ask_size: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub volume: f64,
    #[serde(
        rename = "open_interest",
        deserialize_with = "deserialize_string_to_f64"
    )]
    pub open_interest: f64,
    pub date: String,
    #[serde(
        rename = "implied_volatility",
        deserialize_with = "deserialize_string_to_f64"
    )]
    pub implied_volatility: f64,
    // Add the new fields from your example
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub delta: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub gamma: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub theta: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub vega: f64,
    #[serde(deserialize_with = "deserialize_string_to_f64")]
    pub rho: f64,
}

// Your final OptionsData struct remains mostly the same, but the transformation will change
#[derive(Debug, Clone)]
pub struct OptionsData {
    pub symbol: String,
    pub contract: String,
    pub contract_type: OptionType,
    pub expiration: String,
    pub date: String,
    pub strike: f64,
    pub last: f64,
    pub implied_volatility: f64,
}

// Transform `AlphaVantageOptionsRawResponse` into `Vec<OptionsData>`
pub fn transform_raw_options_to_options_data(
    raw_response: AlphaVantageOptionsRawResponse, // Now takes the new top-level struct
) -> Vec<OptionsData> {
    let mut options_data_vec = Vec::new();

    // Iterate over the `data` field
    for entry in raw_response.data {
        // Determine OptionType from the "type" string
        let contract_type = match entry.option_type_str.as_str() {
            "call" => OptionType::Call,
            "put" => OptionType::Put,
            _ => {
                // Log a warning or skip if an unknown type is encountered
                eprintln!(
                    "Warning: Unknown option type '{}' for contract {}",
                    entry.option_type_str, entry.contract_id
                );
                continue; // Skip this entry
            }
        };

        // Filter out bad data points (e.g., zero implied volatility or last price)
        if entry.implied_volatility > 0.0 && entry.last > 0.0 {
            options_data_vec.push(OptionsData {
                symbol: entry.symbol.clone(), // Use entry.symbol here
                contract: entry.contract_id,  // Use contract_id directly
                contract_type: contract_type,
                expiration: entry.expiration,
                date: entry.date,
                strike: entry.strike,
                last: entry.last,
                implied_volatility: entry.implied_volatility,
            });
        }
    }
    options_data_vec
}

pub async fn options_data(
    key: String,
    symbol: String, // Note: The actual response includes symbol per entry, not necessarily top-level.
    date: String,
) -> Result<Vec<OptionsData>, Box<dyn Error>> {
    let url = format!("https://www.alphavantage.co/query?function=HISTORICAL_OPTIONS&symbol={symbol}&date={date}&apikey={key}");
    let response_text = reqwest::get(&url).await?.text().await?;

    // Now, try to parse into the NEW top-level struct
    match serde_json::from_str::<AlphaVantageOptionsRawResponse>(&response_text) {
        Ok(raw_response) => {
            // Check for empty data array as a soft error/no-data condition
            if raw_response.data.is_empty() {
                eprintln!(
                    "No options data found for symbol {} on date {}. Message: {}",
                    symbol, date, raw_response.message
                );
                return Ok(Vec::new()); // Return empty vector if no data
            }
            Ok(transform_raw_options_to_options_data(raw_response))
        }
        Err(e) => {
            // Still good to print the start/end of the response for hard JSON parsing errors
            eprintln!(
                "Alpha Vantage Options Response (START):\n{:?}",
                &response_text.chars().take(500).collect::<String>()
            );
            eprintln!(
                "Alpha Vantage Options Response (END):\n{:?}",
                &response_text
                    .chars()
                    .rev()
                    .take(500)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>()
            );

            // Check for Alpha Vantage specific error messages within the malformed response
            if let Ok(error_map) = serde_json::from_str::<HashMap<String, String>>(&response_text) {
                if let Some(error_msg) = error_map.get("Error Message") {
                    return Err(
                        format!("Alpha Vantage API Error for {}: {}", date, error_msg).into(),
                    );
                }
                if let Some(note_msg) = error_map.get("Note") {
                    return Err(format!(
                        "Alpha Vantage API Note/Warning for {}: {}",
                        date, note_msg
                    )
                    .into());
                }
            }

            Err(format!(
                "Failed to deserialize Alpha Vantage options response for {}: {}. Raw response might be malformed or unexpected. Full error: {}",
                date, symbol, e
            ).into())
        }
    }
}

pub async fn historical_data(key: String, symbol: String) -> Result<Vec<Ohlcv>, Box<dyn Error>> {
    let url = format!(
        "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={symbol}&outputsize=full&apikey={key}"
    );
    let raw_response = reqwest::get(url).await?.json::<RawOhlcvResponse>().await?;

    let ohlcv_vec = transform_raw_data_to_ohlcv_vec(raw_response.time_series_daily)?;

    Ok(ohlcv_vec)
}
