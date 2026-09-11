//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Polygon.io Options Market Data Integration
//! ═══════════════════════════════════════════════════════════════════════════════
//! [DATA LICENSING COMPLIANCE NOTICE]
//! Status: ACTIVE (Internal Quantitative Analytics Use - VPIN / GEX).
//!
//! Provides institutional retrieval of options trades and quotes from Polygon.io
//! for real-time VPIN (toxicity) and Dealer GEX (gamma exposure) microstructure models.
//! Note: Commercial license agreement with Polygon.io is required for external data redistribution.
//! Controlled via `ENABLE_POLYGON=1`.
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Duration as ChronoDuration, NaiveDate};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Default Polygon REST API base URL
pub const DEFAULT_POLYGON_BASE_URL: &str = "https://api.polygon.io";

/// Normalized institutional option trade print for microstructure & VPIN/GEX processing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptionTrade {
    /// Full OCC options contract ticker (e.g., "O:AAPL240119C00150000")
    pub options_ticker: String,
    /// Underlying equity symbol (e.g., "AAPL")
    pub underlying: String,
    /// Expiration date in ISO format (e.g., "2024-01-19")
    pub expiry: String,
    /// Option contract type ("CALL" or "PUT")
    pub option_type: String,
    /// Option strike price in dollars
    pub strike: f64,
    /// Trade execution price per underlying share
    pub price: f64,
    /// Trade size (number of contracts)
    pub size: u64,
    /// Execution timestamp in microseconds UTC (converted from SIP nanoseconds)
    pub timestamp_us: i64,
    /// Exchange venue identifier code
    pub exchange: Option<i32>,
    /// Trade condition modifier codes
    #[serde(default)]
    pub conditions: Vec<i32>,
}

/// Normalized daily aggregate stock bar (OHLCV) for accurate backtest return calculation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DailyBar {
    /// Stock ticker symbol (e.g., "AAPL", "MSFT", "SPY")
    pub ticker: String,
    /// Date formatted as ISO YYYY-MM-DD
    pub date: String,
    /// Bar close timestamp in milliseconds since Unix epoch
    pub timestamp_ms: i64,
    /// Open price in USD
    pub open: f64,
    /// High price in USD
    pub high: f64,
    /// Low price in USD
    pub low: f64,
    /// Close price in USD
    pub close: f64,
    /// Trading volume in shares
    pub volume: f64,
    /// Volume-weighted average price (VWAP) in USD
    pub vwap: f64,
}

/// Raw deserialization model for Polygon `/v2/aggs/ticker/{ticker}/range/1/day/{from}/{to}` response.
#[derive(Debug, Clone, Deserialize)]
pub struct PolygonAggsResponse {
    pub ticker: Option<String>,
    pub query_count: Option<u64>,
    pub results_count: Option<u64>,
    pub adjusted: Option<bool>,
    #[serde(default)]
    pub results: Vec<PolygonAggRaw>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

/// Raw aggregate bar record inside Polygon API response.
#[derive(Debug, Clone, Deserialize)]
pub struct PolygonAggRaw {
    #[serde(default)]
    pub v: f64,
    pub vw: Option<f64>,
    #[serde(default)]
    pub o: f64,
    #[serde(default)]
    pub c: f64,
    #[serde(default)]
    pub h: f64,
    #[serde(default)]
    pub l: f64,
    pub t: Option<i64>,
    pub n: Option<u64>,
}

/// Intermediate raw deserialization model for Polygon `/v3/trades/{options_ticker}` response.
#[derive(Debug, Clone, Deserialize)]
pub struct PolygonTradesResponse {
    #[serde(default)]
    pub results: Vec<PolygonTradeRaw>,
    pub status: Option<String>,
    pub message: Option<String>,
    pub error: Option<String>,
    pub next_url: Option<String>,
}

/// Raw trade record inside Polygon API response.
#[derive(Debug, Clone, Deserialize)]
pub struct PolygonTradeRaw {
    pub sip_timestamp: Option<i64>,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub size: u64,
    pub exchange: Option<i32>,
    #[serde(default)]
    pub conditions: Vec<i32>,
}

/// Parsed metadata extracted from an OCC options ticker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedOptionsContract {
    pub underlying: String,
    pub expiry: String,
    pub option_type: String,
    pub strike: f64,
}

/// Parses a standard OCC options contract symbol (e.g., `O:AAPL240119C00150000` or `AAPL240119C00150000`).
///
/// Format: `[O:]ROOT + YYMMDD + C/P + 8-digit strike (divided by 1000)`
///
/// # Returns
/// `Ok((underlying, expiry_iso, option_type, strike_dollars))`
pub fn parse_options_ticker(ticker: &str) -> Result<(String, String, String, f64), String> {
    let clean = ticker.trim();
    if clean.is_empty() {
        return Err("Options ticker cannot be empty".to_string());
    }

    let s = if clean.starts_with("O:") {
        &clean[2..]
    } else {
        clean
    };

    // Standard OCC specification requires 6 date digits + 1 type char + 8 strike digits = 15 trailing chars
    if s.len() < 16 {
        return Err(format!(
            "Invalid OCC options ticker '{}': length ({}) must be at least 16 characters",
            ticker,
            s.len()
        ));
    }

    let split_idx = s.len() - 15;
    let underlying = s[..split_idx].trim().to_uppercase();
    if underlying.is_empty() {
        return Err(format!(
            "Invalid OCC options ticker '{}': missing underlying symbol",
            ticker
        ));
    }

    let date_str = &s[split_idx..split_idx + 6];
    if !date_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Invalid expiry date '{}' in options ticker '{}': expected 6 digits YYMMDD",
            date_str, ticker
        ));
    }
    let yy = &date_str[0..2];
    let mm = &date_str[2..4];
    let dd = &date_str[4..6];
    let expiry = format!("20{}-{}-{}", yy, mm, dd);

    let type_char = s
        .chars()
        .nth(split_idx + 6)
        .ok_or_else(|| format!("Missing option type in ticker '{}'", ticker))?
        .to_ascii_uppercase();

    let option_type = match type_char {
        'C' => "CALL".to_string(),
        'P' => "PUT".to_string(),
        _ => {
            return Err(format!(
                "Invalid option type '{}' in ticker '{}': expected 'C' (Call) or 'P' (Put)",
                type_char, ticker
            ))
        }
    };

    let strike_str = &s[split_idx + 7..];
    if strike_str.len() != 8 || !strike_str.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "Invalid strike format '{}' in options ticker '{}': expected exactly 8 digits",
            strike_str, ticker
        ));
    }

    let strike_int: u64 = strike_str
        .parse()
        .map_err(|e| format!("Failed to parse strike number from '{}': {}", strike_str, e))?;
    let strike = (strike_int as f64) / 1000.0;

    Ok((underlying, expiry, option_type, strike))
}

/// Asynchronous Polygon.io Market Data Client.
#[derive(Clone, Debug)]
pub struct PolygonClient {
    api_key: String,
    base_url: String,
    client: Client,
}

impl PolygonClient {
    /// Creates a new `PolygonClient` with the designated API key and default base URL.
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            base_url: DEFAULT_POLYGON_BASE_URL.to_string(),
            client,
        }
    }

    /// Creates a `PolygonClient` with custom base URL (useful for staging / local mocking).
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key,
            base_url,
            client,
        }
    }

    /// Instantiates `PolygonClient` reading `POLYGON_API_KEY` from the runtime environment.
    pub fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("POLYGON_API_KEY")
            .map_err(|_| "POLYGON_API_KEY environment variable is not set".to_string())?;

        let clean_key = api_key.trim().to_string();
        if clean_key.is_empty() {
            return Err("POLYGON_API_KEY environment variable is empty".to_string());
        }

        info!("[PolygonClient] Initialized Polygon client with API key: [MASKED]");
        Ok(Self::new(clean_key))
    }

    /// Fetches historical or intraday options trade prints for an OCC options contract on a given date.
    ///
    /// # Arguments
    /// * `options_ticker` - Standard OCC options contract symbol (e.g., `O:AAPL240119C00150000`)
    /// * `date` - Date string in `YYYY-MM-DD` format
    /// * `limit` - Maximum number of trades to retrieve per page (max 50,000)
    pub async fn fetch_options_trades(
        &self,
        options_ticker: &str,
        date: &str,
        limit: u32,
    ) -> Result<Vec<OptionTrade>, String> {
        // Validate and parse OCC ticker components
        let (underlying, expiry, option_type, strike) = parse_options_ticker(options_ticker)?;

        // Fast-path mock mode for offline testing / CI environments
        if crate::is_production_mode() {
            if self.api_key == "mock_key" || self.api_key.starts_with("mock") || self.api_key.trim().is_empty() {
                return Err("Real Polygon.io API key is required in production mode.".to_string());
            }
        } else if self.api_key == "mock_key"
            || self.api_key.starts_with("mock")
            || std::env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1")
            || std::env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1")
        {
            debug!(
                "[PolygonClient Mock] Generating synthetic option trades for {} on {}",
                options_ticker, date
            );
            return Ok(vec![
                OptionTrade {
                    options_ticker: options_ticker.to_string(),
                    underlying: underlying.clone(),
                    expiry: expiry.clone(),
                    option_type: option_type.clone(),
                    strike,
                    price: 4.55,
                    size: 25,
                    timestamp_us: 1705674600000000,
                    exchange: Some(302),
                    conditions: vec![209],
                },
                OptionTrade {
                    options_ticker: options_ticker.to_string(),
                    underlying,
                    expiry,
                    option_type,
                    strike,
                    price: 4.60,
                    size: 50,
                    timestamp_us: 1705674605000000,
                    exchange: Some(302),
                    conditions: vec![209],
                },
            ]);
        }

        let clean_ticker = options_ticker.trim();
        let formatted_ticker = if clean_ticker.starts_with("O:") {
            clean_ticker.to_string()
        } else {
            format!("O:{}", clean_ticker)
        };

        let endpoint = format!(
            "{}/v3/trades/{}?timestamp.gte={}&limit={}&apiKey={}",
            self.base_url.trim_end_matches('/'),
            formatted_ticker,
            date,
            limit,
            self.api_key
        );

        debug!(
            "[PolygonClient] Fetching options trades for contract '{}' on date '{}' (limit: {})",
            formatted_ticker, date, limit
        );

        let mut backoff = Duration::from_millis(200);
        let max_retries = 2;

        for attempt in 0..=max_retries {
            match self.client.get(&endpoint).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let body_text = resp
                            .text()
                            .await
                            .map_err(|e| format!("Failed to read Polygon response body: {}", e))?;

                        let parsed: PolygonTradesResponse = serde_json::from_str(&body_text)
                            .map_err(|e| {
                                format!(
                                    "Failed to parse Polygon trades JSON (status: {}): {}",
                                    status, e
                                )
                            })?;

                        let trades = parsed
                            .results
                            .into_iter()
                            .map(|raw| OptionTrade {
                                options_ticker: formatted_ticker.clone(),
                                underlying: underlying.clone(),
                                expiry: expiry.clone(),
                                option_type: option_type.clone(),
                                strike,
                                price: raw.price,
                                size: raw.size,
                                timestamp_us: raw.sip_timestamp.map(|ns| ns / 1_000).unwrap_or(0),
                                exchange: raw.exchange,
                                conditions: raw.conditions,
                            })
                            .collect();

                        return Ok(trades);
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        warn!(
                            "[PolygonClient] Rate limited or server error ({}) on attempt {}/{}. Retrying...",
                            status, attempt + 1, max_retries + 1
                        );
                        if attempt < max_retries {
                            sleep(backoff).await;
                            backoff *= 2;
                            continue;
                        }
                        return Err(format!(
                            "Polygon API returned status {} after retries",
                            status
                        ));
                    } else {
                        let err_body = resp.text().await.unwrap_or_default();
                        error!("[PolygonClient] HTTP {} error: {}", status, err_body);
                        return Err(format!(
                            "Polygon API error (status {}): {}",
                            status, err_body
                        ));
                    }
                }
                Err(e) => {
                    warn!(
                        "[PolygonClient] Network error on attempt {}/{}: {}. Retrying...",
                        attempt + 1,
                        max_retries + 1,
                        e
                    );
                    if attempt < max_retries {
                        sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(format!("Failed to connect to Polygon API: {}", e));
                }
            }
        }

        Err("Polygon options trade query failed after maximum retries".to_string())
    }

    /// Fetches historical daily aggregate stock bars (OHLCV) for a given equity ticker and date range.
    ///
    /// # Arguments
    /// * `ticker` - Equity stock ticker symbol (e.g., "AAPL", "MSFT", "SPY")
    /// * `start_date` - Start date in `YYYY-MM-DD` format
    /// * `end_date` - End date in `YYYY-MM-DD` format
    pub async fn fetch_daily_bars(
        &self,
        ticker: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DailyBar>, String> {
        let clean_ticker = ticker.trim().to_uppercase();
        if clean_ticker.is_empty() {
            return Err("Ticker cannot be empty".to_string());
        }

        let parsed_start =
            NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid start_date '{}', expected YYYY-MM-DD: {}",
                    start_date, e
                )
            })?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|e| {
            format!(
                "Invalid end_date '{}', expected YYYY-MM-DD: {}",
                end_date, e
            )
        })?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        // Fast-path mock mode for offline testing / CI environments
        if crate::is_production_mode() {
            if self.api_key == "mock_key" || self.api_key.starts_with("mock") || self.api_key.trim().is_empty() {
                return Err("Real Polygon.io API key is required in production mode.".to_string());
            }
        } else if self.api_key == "mock_key"
            || self.api_key.starts_with("mock")
            || std::env::var("POLYGON_MOCK_MODE").as_deref() == Ok("1")
            || std::env::var("POLYGON_MOCK_FALLBACK").as_deref() == Ok("1")
        {
            debug!(
                "[PolygonClient Mock] Generating synthetic daily OHLCV bars for {} from {} to {}",
                clean_ticker, start_date, end_date
            );
            let mut bars = Vec::new();
            let mut curr = parsed_start;
            let mut day_idx = 0usize;

            let hash_val = clean_ticker.bytes().map(|b| b as usize).sum::<usize>();
            let mut close = match clean_ticker.as_str() {
                "AAPL" => 224.50,
                "NVDA" => 125.00,
                "MSFT" => 415.00,
                "GOOGL" | "GOOG" => 165.00,
                "AMZN" => 185.00,
                "META" => 510.00,
                "TSLA" => 210.00,
                "SPY" => 550.00,
                _ => 100.0 + ((hash_val % 300) as f64),
            };

            while curr <= parsed_end {
                let phase = ((hash_val + day_idx * 17) as f64) * 0.11;
                let daily_change_pct = (phase.sin() * 0.02) + 0.0003;
                let open = close;
                close = (open * (1.0 + daily_change_pct)).max(1.0);
                let high = open.max(close) * (1.0 + (phase.cos().abs() * 0.008));
                let low = open.min(close) * (1.0 - (phase.sin().abs() * 0.008));
                let volume = 25_000_000.0 + ((phase.cos().abs() * 50_000_000.0).round());
                let vwap = (open + high + low + close) / 4.0;
                let ts_ms = curr
                    .and_hms_opt(16, 0, 0)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis();

                bars.push(DailyBar {
                    ticker: clean_ticker.clone(),
                    date: curr.format("%Y-%m-%d").to_string(),
                    timestamp_ms: ts_ms,
                    open: (open * 100.0).round() / 100.0,
                    high: (high * 100.0).round() / 100.0,
                    low: (low * 100.0).round() / 100.0,
                    close: (close * 100.0).round() / 100.0,
                    volume,
                    vwap: (vwap * 100.0).round() / 100.0,
                });

                curr += ChronoDuration::days(1);
                day_idx += 1;
            }

            return Ok(bars);
        }

        let endpoint = format!(
            "{}/v2/aggs/ticker/{}/range/1/day/{}/{}?adjusted=true&sort=asc&limit=50000&apiKey={}",
            self.base_url.trim_end_matches('/'),
            clean_ticker,
            start_date,
            end_date,
            self.api_key
        );

        debug!(
            "[PolygonClient] Fetching daily bars for ticker '{}' from '{}' to '{}'",
            clean_ticker, start_date, end_date
        );

        let mut backoff = Duration::from_millis(200);
        let max_retries = 2;

        for attempt in 0..=max_retries {
            match self.client.get(&endpoint).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let body_text = resp
                            .text()
                            .await
                            .map_err(|e| format!("Failed to read Polygon response body: {}", e))?;

                        let parsed: PolygonAggsResponse = serde_json::from_str(&body_text)
                            .map_err(|e| {
                                format!(
                                    "Failed to parse Polygon aggs JSON (status: {}): {}",
                                    status, e
                                )
                            })?;

                        let bars = parsed
                            .results
                            .into_iter()
                            .map(|raw| {
                                let ts_ms = raw.t.unwrap_or(0);
                                let date_str = DateTime::from_timestamp_millis(ts_ms)
                                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                                    .unwrap_or_else(|| start_date.to_string());

                                DailyBar {
                                    ticker: clean_ticker.clone(),
                                    date: date_str,
                                    timestamp_ms: ts_ms,
                                    open: raw.o,
                                    high: raw.h,
                                    low: raw.l,
                                    close: raw.c,
                                    volume: raw.v,
                                    vwap: raw.vw.unwrap_or(raw.c),
                                }
                            })
                            .collect();

                        return Ok(bars);
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        warn!(
                            "[PolygonClient] Rate limited or server error ({}) on attempt {}/{}. Retrying...",
                            status, attempt + 1, max_retries + 1
                        );
                        if attempt < max_retries {
                            sleep(backoff).await;
                            backoff *= 2;
                            continue;
                        }
                        return Err(format!(
                            "Polygon API returned status {} after retries",
                            status
                        ));
                    } else {
                        let err_body = resp.text().await.unwrap_or_default();
                        error!("[PolygonClient] HTTP {} error: {}", status, err_body);
                        return Err(format!(
                            "Polygon API error (status {}): {}",
                            status, err_body
                        ));
                    }
                }
                Err(e) => {
                    warn!(
                        "[PolygonClient] Network error on attempt {}/{}: {}. Retrying...",
                        attempt + 1,
                        max_retries + 1,
                        e
                    );
                    if attempt < max_retries {
                        sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(format!("Failed to connect to Polygon API: {}", e));
                }
            }
        }

        Err("Polygon daily bars query failed after maximum retries".to_string())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_parse_options_ticker_standard_aapl() {
        let (underlying, expiry, option_type, strike) =
            parse_options_ticker("O:AAPL240119C00150000")
                .expect("Failed to parse standard AAPL ticker");

        assert_eq!(underlying, "AAPL");
        assert_eq!(expiry, "2024-01-19");
        assert_eq!(option_type, "CALL");
        assert!((strike - 150.0).abs() < 1e-6);
    }

    #[test]
    fn test_parse_options_ticker_without_o_prefix_put() {
        let (underlying, expiry, option_type, strike) =
            parse_options_ticker("SPY240621P00520500").expect("Failed to parse SPY Put ticker");

        assert_eq!(underlying, "SPY");
        assert_eq!(expiry, "2024-06-21");
        assert_eq!(option_type, "PUT");
        assert!((strike - 520.50).abs() < 1e-6);
    }

    #[test]
    fn test_parse_options_ticker_fractional_strike_nvda() {
        let (underlying, expiry, option_type, strike) =
            parse_options_ticker("O:NVDA251219C00125750")
                .expect("Failed to parse NVDA fractional strike");

        assert_eq!(underlying, "NVDA");
        assert_eq!(expiry, "2025-12-19");
        assert_eq!(option_type, "CALL");
        assert!((strike - 125.75).abs() < 1e-6);
    }

    #[test]
    fn test_parse_options_ticker_invalid_formats() {
        // Empty ticker
        assert!(parse_options_ticker("").is_err());
        // Too short
        assert!(parse_options_ticker("O:AAPL").is_err());
        // Invalid option type (neither C nor P)
        assert!(parse_options_ticker("O:AAPL240119X00150000").is_err());
        // Invalid non-numeric date
        assert!(parse_options_ticker("O:AAPL24XX19C00150000").is_err());
        // Invalid non-numeric strike
        assert!(parse_options_ticker("O:AAPL240119C0015000A").is_err());
    }

    #[test]
    fn test_polygon_client_from_env_or_missing() {
        // Test with explicit key
        let client = PolygonClient::new("test_api_key_123".to_string());
        assert_eq!(client.api_key, "test_api_key_123");
        assert_eq!(client.base_url, DEFAULT_POLYGON_BASE_URL);

        // Test from_env when variable is set or unset
        let key_backup = std::env::var("POLYGON_API_KEY").ok();
        std::env::set_var("POLYGON_API_KEY", "custom_poly_key_456");
        let client_env = PolygonClient::from_env().expect("Should load valid key from env");
        assert_eq!(client_env.api_key, "custom_poly_key_456");

        // Clean up / restore
        if let Some(k) = key_backup {
            std::env::set_var("POLYGON_API_KEY", k);
        } else {
            std::env::remove_var("POLYGON_API_KEY");
        }
    }

    #[tokio::test]
    async fn test_polygon_client_mock_fetch_trades() {
        std::env::set_var("POLYGON_MOCK_MODE", "1");
        let client = PolygonClient::new("mock_key".to_string());

        let trades = client
            .fetch_options_trades("O:AAPL240119C00150000", "2024-01-19", 100)
            .await
            .expect("Mock fetch trades failed");

        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].underlying, "AAPL");
        assert_eq!(trades[0].option_type, "CALL");
        assert_eq!(trades[0].strike, 150.0);
        assert_eq!(trades[0].price, 4.55);
        assert_eq!(trades[0].size, 25);
        assert_eq!(trades[1].price, 4.60);
        assert_eq!(trades[1].size, 50);

        std::env::remove_var("POLYGON_MOCK_MODE");
    }

    #[tokio::test]
    async fn test_polygon_client_mock_fetch_daily_bars() {
        std::env::set_var("POLYGON_MOCK_MODE", "1");
        let client = PolygonClient::new("mock_key".to_string());

        let bars = client
            .fetch_daily_bars("AAPL", "2025-01-01", "2025-01-05")
            .await
            .expect("Mock fetch daily bars failed");

        assert_eq!(bars.len(), 5);
        assert_eq!(bars[0].ticker, "AAPL");
        assert_eq!(bars[0].date, "2025-01-01");
        assert!(bars[0].open > 0.0);
        assert!(bars[0].close > 0.0);
        assert!(bars[0].high >= bars[0].low);
        assert!(bars[0].volume > 0.0);

        std::env::remove_var("POLYGON_MOCK_MODE");
    }
}
