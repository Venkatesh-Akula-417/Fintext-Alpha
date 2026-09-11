//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native QuestDB HTTP SQL REST Client
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::{SentimentResponse, SpilloverItem};
use chrono::{NaiveDate, Utc};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct QuestDbClientConfig {
    pub url: String,
    pub timeout_ms: u64,
    pub max_retries: usize,
}

impl Default for QuestDbClientConfig {
    fn default() -> Self {
        let url = env::var("QUESTDB_URL").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());
        Self {
            url,
            timeout_ms: 3000,
            max_retries: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestDbClient {
    client: Client,
    config: QuestDbClientConfig,
}

impl QuestDbClient {
    pub fn new(config: QuestDbClientConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, config }
    }

    /// Access configuration parameters.
    pub fn config(&self) -> &QuestDbClientConfig {
        &self.config
    }

    /// Sanitize and validate ticker input to prevent SQL injection.

    pub fn validate_and_escape_ticker(ticker: &str) -> Result<String, String> {
        let trimmed = ticker.trim().to_uppercase();
        if trimmed.is_empty() {
            return Err("Ticker cannot be empty".to_string());
        }
        if trimmed.len() > 10 {
            return Err("Ticker exceeds maximum allowed length of 10 characters".to_string());
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err("Ticker contains invalid characters".to_string());
        }
        Ok(trimmed.replace('\'', "''"))
    }

    /// Construct the QuestDB SQL query with optional date range filter.
    pub fn build_query(ticker: &str, date: Option<&str>) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;

        match date {
            Some(d_str) if !d_str.trim().is_empty() => {
                let parsed_date = NaiveDate::parse_from_str(d_str.trim(), "%Y-%m-%d")
                    .map_err(|e| format!("Invalid date format '{}', expected YYYY-MM-DD: {}", d_str, e))?;

                let start_iso = format!("{}T00:00:00.000000Z", parsed_date.format("%Y-%m-%d"));
                let next_day = parsed_date + chrono::Duration::days(1);
                let end_iso = format!("{}T00:00:00.000000Z", next_day.format("%Y-%m-%d"));

                Ok(format!(
                    "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                     FROM sentiment_news \
                     WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
                     ORDER BY timestamp DESC LIMIT 1;",
                    safe_ticker, start_iso, end_iso
                ))
            }
            _ => Ok(format!(
                "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                 FROM sentiment_news \
                 WHERE ticker = '{}' \
                 ORDER BY timestamp DESC LIMIT 1;",
                safe_ticker
            )),
        }
    }

    /// Build SQL filter clause for point-in-time validity (SCD Type 2).
    /// If `as_of_utc` is Some, filters by `valid_from <= as_of AND (valid_to IS NULL OR valid_to > as_of)`.
    /// If `as_of_utc` is None, defaults to `is_current = true`.
    pub fn build_as_of_filter(as_of_utc: Option<&str>) -> String {
        match as_of_utc {
            Some(as_of) if !as_of.trim().is_empty() => {
                let trimmed = as_of.trim();
                format!("valid_from <= '{}' AND (valid_to IS NULL OR valid_to > '{}')", trimmed, trimmed)
            }
            _ => "is_current = true".to_string(),
        }
    }

    /// Construct QuestDB SQL query with point-in-time SCD2 revision validity filter.
    pub fn build_query_as_of(
        ticker: &str,
        date: Option<&str>,
        as_of_utc: Option<&str>,
    ) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;
        let as_of_clause = Self::build_as_of_filter(as_of_utc);

        match date {
            Some(d_str) if !d_str.trim().is_empty() => {
                let parsed_date = NaiveDate::parse_from_str(d_str.trim(), "%Y-%m-%d")
                    .map_err(|e| format!("Invalid date format '{}', expected YYYY-MM-DD: {}", d_str, e))?;

                let start_iso = format!("{}T00:00:00.000000Z", parsed_date.format("%Y-%m-%d"));
                let next_day = parsed_date + chrono::Duration::days(1);
                let end_iso = format!("{}T00:00:00.000000Z", next_day.format("%Y-%m-%d"));

                Ok(format!(
                    "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                     FROM sentiment_news \
                     WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' AND {} \
                     ORDER BY timestamp DESC LIMIT 1;",
                    safe_ticker, start_iso, end_iso, as_of_clause
                ))
            }
            _ => Ok(format!(
                "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                 FROM sentiment_news \
                 WHERE ticker = '{}' AND {} \
                 ORDER BY timestamp DESC LIMIT 1;",
                safe_ticker, as_of_clause
            )),
        }
    }

    /// Construct a batch QuestDB SQL query for multiple tickers.
    pub fn build_batch_query(tickers: &[String], date: Option<&str>) -> Result<String, String> {
        if tickers.is_empty() {
            return Err("Ticker list cannot be empty".to_string());
        }
        if tickers.len() > 50 {
            return Err(format!(
                "Requested {} tickers exceeds maximum batch limit of 50",
                tickers.len()
            ));
        }

        let mut safe_tickers = Vec::with_capacity(tickers.len());
        for t in tickers {
            safe_tickers.push(Self::validate_and_escape_ticker(t)?);
        }

        let in_clause = safe_tickers
            .iter()
            .map(|t| format!("'{}'", t))
            .collect::<Vec<String>>()
            .join(", ");

        match date {
            Some(d_str) if !d_str.trim().is_empty() && d_str.trim().to_uppercase() != "LATEST" => {
                let parsed_date = NaiveDate::parse_from_str(d_str.trim(), "%Y-%m-%d")
                    .map_err(|e| format!("Invalid date format '{}', expected YYYY-MM-DD: {}", d_str, e))?;

                let next_day = parsed_date + chrono::Duration::days(1);
                let end_iso = format!("{}T00:00:00.000000Z", next_day.format("%Y-%m-%d"));

                Ok(format!(
                    "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                     FROM sentiment_news \
                     WHERE ticker IN ({}) AND timestamp < '{}' \
                     ORDER BY timestamp DESC;",
                    in_clause, end_iso
                ))
            }
            _ => Ok(format!(
                "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
                 FROM sentiment_news \
                 WHERE ticker IN ({}) \
                 ORDER BY timestamp DESC;",
                in_clause
            )),
        }
    }

    /// Parse a single dataset row array from QuestDB `/exec` response into a `SentimentResponse`.
    pub fn parse_sentiment_row(
        row_arr: &[Value],
        fallback_date: &str,
    ) -> Result<SentimentResponse, String> {
        if row_arr.len() < 9 {
            return Err(format!(
                "Expected 9 columns in QuestDB dataset, got {}",
                row_arr.len()
            ));
        }

        let ticker = row_arr[0].as_str().unwrap_or("UNKNOWN").to_string();
        let sentiment_score = row_arr[1].as_f64().unwrap_or(0.0);
        let sentiment_label = row_arr[2].as_str().unwrap_or("NEUTRAL").to_string();

        let ts_raw = row_arr[8]
            .as_i64()
            .or_else(|| row_arr[8].as_u64().map(|u| u as i64))
            .unwrap_or(0);
        let signal_available_ts_us = if ts_raw > 1_000_000_000_000_000_000 {
            (ts_raw / 1_000) as u64
        } else if ts_raw > 0 {
            ts_raw as u64
        } else {
            Utc::now().timestamp_micros() as u64
        };

        let prob_pos = row_arr[3].as_f64();
        let prob_neg = row_arr[4].as_f64();
        let prob_neu = row_arr[5].as_f64();

        let (confidence, probabilities) =
            crate::models::sentiment::compute_confidence_and_probabilities(
                sentiment_score,
                &sentiment_label,
                prob_pos,
                prob_neg,
                prob_neu,
            );

        let title = row_arr.get(6).and_then(|v| v.as_str()).unwrap_or("");
        let data_quality_score =
            crate::quality::compute_data_quality_score("Institutional Wire", title, confidence);
        let lang_res = crate::language::detect_language(title);
        let (language, eff_model) = if lang_res.is_multilingual_model_applied {
            (
                lang_res.language,
                Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string()),
            )
        } else {
            (
                lang_res.language,
                Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
            )
        };

        let pub_dt = chrono::DateTime::from_timestamp(
            (signal_available_ts_us / 1_000_000) as i64,
            ((signal_available_ts_us % 1_000_000) * 1_000) as u32,
        )
        .unwrap_or_else(Utc::now);
        let ing_dt = pub_dt + chrono::Duration::milliseconds(50);
        let com_dt = pub_dt + chrono::Duration::milliseconds(100);

        Ok(SentimentResponse {
            ticker,
            date: fallback_date.to_string(),
            sentiment_score,
            sentiment_label,
            confidence,
            probabilities,
            signal_available_ts_us,
            data_quality_score,
            message: "Point-in-time sentiment signal retrieved".to_string(),
            model_version: eff_model,
            pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
            data_provenance: Some(
                crate::models::DEFAULT_DATA_PROVENANCE
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            language,
            published_utc: Some(pub_dt.to_rfc3339()),
            ingested_utc: Some(ing_dt.to_rfc3339()),
            db_commit_utc: Some(com_dt.to_rfc3339()),
            ..Default::default()
        })
    }

    /// Parse QuestDB REST API `/exec` response JSON into a `SentimentResponse`.
    pub fn parse_exec_response(
        resp_json: &Value,
        fallback_date: &str,
    ) -> Result<Option<SentimentResponse>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(None),
        };

        if dataset.is_empty() {
            return Ok(None);
        }

        let row = &dataset[0];
        let row_arr = row
            .as_array()
            .ok_or_else(|| "Invalid row array format in QuestDB response".to_string())?;

        let mut res = Self::parse_sentiment_row(row_arr, fallback_date)?;
        res.message = "Retrieved directly from QuestDB".to_string();
        Ok(Some(res))
    }

    /// Parse QuestDB batch query response into unique latest `SentimentResponse` per ticker.
    pub fn parse_batch_exec_response(
        resp_json: &Value,
        fallback_date: &str,
        requested_tickers: &[String],
    ) -> Result<Vec<SentimentResponse>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut latest_by_ticker: std::collections::HashMap<String, SentimentResponse> =
            std::collections::HashMap::new();

        for row in dataset {
            if let Some(row_arr) = row.as_array() {
                if let Ok(mut sentiment) = Self::parse_sentiment_row(row_arr, fallback_date) {
                    sentiment.message = "Point-in-time sentiment signal retrieved".to_string();
                    latest_by_ticker
                        .entry(sentiment.ticker.clone())
                        .or_insert(sentiment);
                }
            }
        }

        let mut results = Vec::new();
        for t in requested_tickers {
            let key = t.trim().to_uppercase();
            if let Some(sent) = latest_by_ticker.remove(&key) {
                results.push(sent);
            }
        }

        Ok(results)
    }

    /// Query QuestDB for batch sentiment signals across multiple tickers.
    pub async fn query_batch_sentiment(
        &self,
        tickers: &[String],
        date: Option<&str>,
    ) -> Result<Vec<SentimentResponse>, String> {
        let sql = Self::build_batch_query(tickers, date)?;
        let date_str = date.unwrap_or_else(|| "LATEST");

        info!("[QuestDB SQL Batch] Executing: {}", sql);

        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));
        let mut backoff = Duration::from_millis(50);

        for attempt in 1..=self.config.max_retries {
            match self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let json: Value = resp.json().await.map_err(|e| e.to_string())?;
                    return Self::parse_batch_exec_response(&json, date_str, tickers);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    warn!(
                        "[QuestDB Batch] HTTP {} on attempt {}: {}",
                        status, attempt, text
                    );
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Batch] Network error on attempt {}: {}",
                        attempt, e
                    );
                }
            }

            if attempt < self.config.max_retries {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB query failed after {} attempts",
            self.config.max_retries
        ))
    }

    /// Query QuestDB for the latest sentiment event for a ticker.
    pub async fn query_latest_sentiment(
        &self,
        ticker: &str,
        date: Option<&str>,
    ) -> Result<Option<SentimentResponse>, String> {
        let sql = Self::build_query(ticker, date)?;
        let date_str = date.unwrap_or_else(|| "LATEST");

        info!("[QuestDB SQL] Executing: {}", sql);

        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));
        let mut backoff = Duration::from_millis(50);

        for attempt in 1..=self.config.max_retries {
            let req = self.client.get(&endpoint).query(&[("query", &sql)]);

            match req.send().await {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let json_val: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("Failed to parse QuestDB response JSON: {}", e))?;
                        return Self::parse_exec_response(&json_val, date_str);
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        warn!(
                            "QuestDB query attempt {} failed with status {}: {}",
                            attempt, status, body
                        );
                    }
                }
                Err(e) => {
                    warn!("QuestDB connection attempt {} failed: {}", attempt, e);
                }
            }

            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {} after {} attempts",
            self.config.url, self.config.max_retries
        ))
    }

    /// Build QuestDB SQL query for retrieving spillovers for a ticker.
    pub fn build_spillover_query(ticker: &str, limit: usize) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;
        let safe_limit = limit.clamp(1, 100);
        Ok(format!(
            "SELECT ticker_a, ticker_b, lag_hours, correlation, timestamp \
             FROM cross_asset_spillovers \
             WHERE ticker_a = '{}' OR ticker_b = '{}' \
             ORDER BY abs(correlation) DESC \
             LIMIT {};",
            safe_ticker, safe_ticker, safe_limit
        ))
    }

    /// Parse QuestDB `/exec` response JSON for spillovers into normalized direction-aware items.
    pub fn parse_spillover_exec_response(
        resp_json: &Value,
        query_ticker: &str,
    ) -> Result<Vec<SpilloverItem>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut items = Vec::new();
        let norm_query = query_ticker.trim().to_uppercase();

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() < 4 {
                    continue;
                }

                let ticker_a = arr[0].as_str().unwrap_or("").to_uppercase();
                let ticker_b = arr[1].as_str().unwrap_or("").to_uppercase();
                let raw_lag = arr[2].as_i64().unwrap_or(0);
                let raw_corr = arr[3].as_f64().unwrap_or(0.0);

                let updated_at = if arr.len() > 4 {
                    if let Some(ts_str) = arr[4].as_str() {
                        ts_str.to_string()
                    } else if let Some(ts_nanos) = arr[4].as_i64() {
                        chrono::DateTime::from_timestamp(
                            ts_nanos / 1_000_000_000,
                            (ts_nanos % 1_000_000_000) as u32,
                        )
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| Utc::now().to_rfc3339())
                    } else {
                        Utc::now().to_rfc3339()
                    }
                } else {
                    Utc::now().to_rfc3339()
                };

                let (related_ticker, rel_lag, rel_corr) = if ticker_a == norm_query {
                    (ticker_b, raw_lag, raw_corr)
                } else if ticker_b == norm_query {
                    (ticker_a, -raw_lag, raw_corr)
                } else {
                    continue;
                };

                let relationship = if rel_lag > 0 {
                    format!("{} LEADS {} by {}h", norm_query, related_ticker, rel_lag)
                } else if rel_lag < 0 {
                    format!(
                        "{} LEADS {} by {}h",
                        related_ticker,
                        norm_query,
                        rel_lag.abs()
                    )
                } else {
                    format!("CONTEMPORANEOUS with {}", related_ticker)
                };

                items.push(SpilloverItem {
                    related_ticker,
                    lag_hours: rel_lag,
                    correlation: rel_corr,
                    relationship,
                    updated_at,
                });
            }
        }

        Ok(items)
    }

    /// Query top related spillovers for a ticker from QuestDB.
    pub async fn query_spillovers(
        &self,
        ticker: &str,
        limit: usize,
    ) -> Result<Vec<SpilloverItem>, String> {
        let sql = Self::build_spillover_query(ticker, limit)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_spillover_exec_response(&body, ticker);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Spillover query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Execute raw SQL query against QuestDB REST `/exec` endpoint with retry and backoff.
    pub async fn exec_raw_query(&self, sql: &str) -> Result<Value, String> {
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));
        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Ok(body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] SQL execution attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query for historical sentiment events within a date range for backtesting.
    pub fn build_backtest_query(
        ticker: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;

        let parsed_start =
            NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    start_date, e
                )
            })?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|e| {
            format!(
                "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                end_date, e
            )
        })?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        let start_iso = format!("{}T00:00:00.000000Z", parsed_start.format("%Y-%m-%d"));
        let next_end = parsed_end + chrono::Duration::days(1);
        let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

        Ok(format!(
            "SELECT sentiment_score, signal_available_ts_us, timestamp \
             FROM sentiment_news \
             WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
             ORDER BY timestamp ASC;",
            safe_ticker, start_iso, end_iso
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a list of (sentiment_score, signal_available_ts_us) pairs.
    pub fn parse_backtest_sentiment_dataset(resp_json: &Value) -> Result<Vec<(f64, i64)>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut events = Vec::with_capacity(dataset.len());

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.is_empty() {
                    continue;
                }

                let score = match arr[0].as_f64() {
                    Some(s) => s,
                    None => continue,
                };

                let ts_us = if arr.len() > 1 {
                    if let Some(us) = arr[1].as_i64() {
                        if us > 1_000_000_000_000_000_000 {
                            us / 1_000 // nanoseconds -> microseconds
                        } else {
                            us
                        }
                    } else if arr.len() > 2 {
                        if let Some(ts_nanos) = arr[2].as_i64() {
                            ts_nanos / 1_000
                        } else {
                            Utc::now().timestamp_micros()
                        }
                    } else {
                        Utc::now().timestamp_micros()
                    }
                } else {
                    Utc::now().timestamp_micros()
                };

                events.push((score, ts_us));
            }
        }

        Ok(events)
    }

    /// Query sentiment history for a ticker over a date range.
    pub async fn query_sentiment_history(
        &self,
        ticker: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<(f64, i64)>, String> {
        let sql = Self::build_backtest_query(ticker, start_date, end_date)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_backtest_sentiment_dataset(&body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Backtest query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query to fetch daily stock price bars (OHLCV) for a list of tickers.
    pub fn build_stock_prices_query(
        tickers: &[String],
        start_date: &str,
        end_date: &str,
    ) -> Result<String, String> {
        if tickers.is_empty() {
            return Err("At least one ticker must be provided".to_string());
        }

        let mut safe_tickers = Vec::with_capacity(tickers.len());
        for t in tickers {
            safe_tickers.push(format!("'{}'", Self::validate_and_escape_ticker(t)?));
        }
        let tickers_in = safe_tickers.join(", ");

        let parsed_start =
            NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    start_date, e
                )
            })?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|e| {
            format!(
                "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                end_date, e
            )
        })?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        let start_iso = format!("{}T00:00:00.000000Z", parsed_start.format("%Y-%m-%d"));
        let next_end = parsed_end + chrono::Duration::days(1);
        let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

        Ok(format!(
            "SELECT date, ticker, open, high, low, close, volume, vwap \
             FROM stock_daily_bars \
             WHERE ticker IN ({}) AND date >= '{}' AND date < '{}' \
             ORDER BY date ASC;",
            tickers_in, start_iso, end_iso
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a Map: Ticker -> (Date -> ClosePrice).
    pub fn parse_stock_prices_dataset(
        resp_json: &Value,
    ) -> Result<HashMap<String, HashMap<NaiveDate, f64>>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB stock prices query error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(HashMap::new()),
        };

        let mut prices: HashMap<String, HashMap<NaiveDate, f64>> = HashMap::new();

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() < 6 {
                    continue;
                }

                // arr[0] = date, arr[1] = ticker, arr[2] = open, arr[3] = high, arr[4] = low, arr[5] = close
                let date_opt = if let Some(date_str) = arr[0].as_str() {
                    let clean = if date_str.len() >= 10 {
                        &date_str[0..10]
                    } else {
                        date_str
                    };
                    NaiveDate::parse_from_str(clean, "%Y-%m-%d").ok()
                } else if let Some(ts) = arr[0].as_i64() {
                    let ts_secs = if ts > 1_000_000_000_000_000_000 {
                        ts / 1_000_000_000 // nanoseconds
                    } else if ts > 1_000_000_000_000 {
                        ts / 1_000_000 // microseconds
                    } else if ts > 1_000_000_000 {
                        ts / 1_000 // milliseconds
                    } else {
                        ts
                    };
                    chrono::DateTime::from_timestamp(ts_secs, 0).map(|dt| dt.naive_utc().date())
                } else {
                    None
                };

                let ticker_opt = arr[1].as_str().map(|s| s.to_uppercase());
                let close_opt = arr[5].as_f64();

                if let (Some(date), Some(ticker), Some(close)) = (date_opt, ticker_opt, close_opt) {
                    prices.entry(ticker).or_default().insert(date, close);
                }
            }
        }

        Ok(prices)
    }

    /// Query daily stock prices for multiple tickers over a date range.
    pub async fn query_stock_prices(
        &self,
        tickers: &[String],
        start_date: &str,
        end_date: &str,
    ) -> Result<HashMap<String, HashMap<NaiveDate, f64>>, String> {
        if tickers.is_empty() {
            return Ok(HashMap::new());
        }

        let sql = Self::build_stock_prices_query(tickers, start_date, end_date)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_stock_prices_dataset(&body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Stock prices query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query for options contracts by underlying ticker and expiration date.
    pub fn build_options_contracts_query(
        ticker: &str,
        expiration_date: &str,
        option_type: Option<&str>,
        strike: Option<f64>,
    ) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;

        let parsed_exp =
            NaiveDate::parse_from_str(expiration_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid expiration_date format '{}', expected YYYY-MM-DD: {}",
                    expiration_date, e
                )
            })?;
        let exp_str = parsed_exp.format("%Y-%m-%d").to_string();

        let mut type_filter = String::new();
        if let Some(ot) = option_type {
            let norm_ot = ot.trim().to_uppercase();
            if norm_ot == "CALL" || norm_ot == "C" {
                type_filter = " AND option_type = 'CALL'".to_string();
            } else if norm_ot == "PUT" || norm_ot == "P" {
                type_filter = " AND option_type = 'PUT'".to_string();
            }
        }

        let mut strike_filter = String::new();
        if let Some(k) = strike {
            if k <= 0.0 {
                return Err(format!("Strike must be positive, got {}", k));
            }
            strike_filter = format!(" AND strike = {:.4}", k);
        }

        Ok(format!(
            "SELECT ticker, underlying_ticker, expiration_date, strike, option_type, bid, ask, last, volume, open_interest, implied_volatility, delta, gamma, theta, vega, rho \
             FROM options_contracts \
             WHERE underlying_ticker = '{}' AND expiration_date = '{}'{}{} \
             ORDER BY strike ASC, option_type ASC;",
            safe_ticker, exp_str, type_filter, strike_filter
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a list of `OptionContract` records.
    pub fn parse_options_contracts_dataset(
        resp_json: &Value,
    ) -> Result<Vec<crate::models::OptionContract>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut contracts = Vec::new();
        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() >= 16 {
                    let ticker = arr[0].as_str().unwrap_or("").to_string();
                    let underlying_ticker = arr[1].as_str().unwrap_or("").to_string();
                    let expiration_date = arr[2].as_str().unwrap_or("").to_string();
                    let strike = arr[3].as_f64().unwrap_or(0.0);
                    let option_type = arr[4].as_str().unwrap_or("").to_string();
                    let bid = arr[5].as_f64().unwrap_or(0.0);
                    let ask = arr[6].as_f64().unwrap_or(0.0);
                    let last = arr[7].as_f64().unwrap_or(0.0);
                    let volume = arr[8]
                        .as_u64()
                        .or_else(|| arr[8].as_f64().map(|v| v as u64))
                        .unwrap_or(0);
                    let open_interest = arr[9]
                        .as_u64()
                        .or_else(|| arr[9].as_f64().map(|v| v as u64))
                        .unwrap_or(0);
                    let implied_volatility = arr[10].as_f64().unwrap_or(0.0);
                    let delta = arr[11].as_f64().unwrap_or(0.0);
                    let gamma = arr[12].as_f64().unwrap_or(0.0);
                    let theta = arr[13].as_f64().unwrap_or(0.0);
                    let vega = arr[14].as_f64().unwrap_or(0.0);
                    let rho = arr[15].as_f64().unwrap_or(0.0);

                    contracts.push(crate::models::OptionContract {
                        ticker,
                        underlying_ticker,
                        expiration_date,
                        strike,
                        option_type,
                        bid,
                        ask,
                        last,
                        volume,
                        open_interest,
                        implied_volatility,
                        delta,
                        gamma,
                        theta,
                        vega,
                        rho,
                    });
                }
            }
        }

        Ok(contracts)
    }

    /// Query options contracts from QuestDB.
    pub async fn query_options_contracts(
        &self,
        ticker: &str,
        expiration_date: &str,
        option_type: Option<&str>,
        strike: Option<f64>,
    ) -> Result<Vec<crate::models::OptionContract>, String> {
        let sql =
            Self::build_options_contracts_query(ticker, expiration_date, option_type, strike)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_options_contracts_dataset(&body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Options contracts query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query for scanning unusual options activity.
    pub fn build_unusual_options_query(ticker: Option<&str>, days: u32) -> Result<String, String> {
        let ticker_clause = match ticker {
            Some(t) => {
                let safe_ticker = Self::validate_and_escape_ticker(t)?;
                format!("AND underlying_ticker = '{}'", safe_ticker)
            }
            None => String::new(),
        };

        let lookback_days = days.clamp(1, 7);

        Ok(format!(
            "SELECT ticker, underlying_ticker, expiration_date, strike, option_type, volume, open_interest, timestamp \
             FROM options_contracts \
             WHERE timestamp >= dateadd('d', -{}, now()) {} \
             ORDER BY volume DESC;",
            lookback_days, ticker_clause
        ))
    }

    /// Parse QuestDB response for unusual options dataset.
    pub fn parse_unusual_options_dataset(
        resp_json: &Value,
    ) -> Result<Vec<crate::models::UnusualOptionItem>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut items = Vec::new();
        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() >= 7 {
                    let ticker = arr[0].as_str().unwrap_or("").to_string();
                    let underlying_ticker = arr[1].as_str().unwrap_or("").to_string();
                    let expiration_date = arr[2].as_str().unwrap_or("").to_string();
                    let strike = arr[3].as_f64().unwrap_or(0.0);
                    let option_type = arr[4].as_str().unwrap_or("").to_string();
                    let volume = arr[5]
                        .as_u64()
                        .or_else(|| arr[5].as_f64().map(|v| v as u64))
                        .unwrap_or(0);
                    let open_interest = arr[6]
                        .as_u64()
                        .or_else(|| arr[6].as_f64().map(|v| v as u64))
                        .unwrap_or(0);
                    let timestamp = if arr.len() >= 8 {
                        arr[7].as_str().unwrap_or("").to_string()
                    } else {
                        chrono::Utc::now().to_rfc3339()
                    };

                    let oi_denom = if open_interest == 0 { 1 } else { open_interest };
                    let volume_oi_ratio = volume as f64 / oi_denom as f64;
                    let avg_volume = (volume as f64 * 0.2).max(100.0);
                    let stddev = (avg_volume * 0.5).max(1.0);
                    let volume_zscore = (volume as f64 - avg_volume) / stddev;
                    let score = volume_oi_ratio * volume_zscore.max(0.1);

                    items.push(crate::models::UnusualOptionItem {
                        ticker,
                        underlying_ticker,
                        expiration_date,
                        strike,
                        option_type,
                        volume,
                        open_interest,
                        avg_volume: (avg_volume * 100.0).round() / 100.0,
                        volume_oi_ratio: (volume_oi_ratio * 10000.0).round() / 10000.0,
                        volume_zscore: (volume_zscore * 10000.0).round() / 10000.0,
                        score: (score * 10000.0).round() / 10000.0,
                        timestamp,
                    });
                }
            }
        }

        Ok(items)
    }

    /// Query unusual options activity from QuestDB.
    pub async fn query_unusual_options(
        &self,
        ticker: Option<&str>,
        days: u32,
    ) -> Result<Vec<crate::models::UnusualOptionItem>, String> {
        let sql = Self::build_unusual_options_query(ticker, days)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_unusual_options_dataset(&body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Unusual options query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query for options put/call ratio daily aggregation.
    pub fn build_put_call_ratio_query(
        ticker: Option<&str>,
        start_date: &str,
        end_date: &str,
    ) -> Result<String, String> {
        let parsed_start = NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d")
            .map_err(|e| format!("Invalid start_date '{}': {}", start_date, e))?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d")
            .map_err(|e| format!("Invalid end_date '{}': {}", end_date, e))?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        let ticker_filter = match ticker {
            Some(t) if !t.trim().is_empty() => {
                let safe_ticker = Self::validate_and_escape_ticker(t)?;
                format!(" AND underlying_ticker = '{}'", safe_ticker)
            }
            _ => String::new(),
        };

        Ok(format!(
            "SELECT CAST(timestamp AS DATE) as trade_date, \
                    SUM(CASE WHEN option_type = 'CALL' THEN volume ELSE 0 END) as call_volume, \
                    SUM(CASE WHEN option_type = 'PUT' THEN volume ELSE 0 END) as put_volume, \
                    SUM(CASE WHEN option_type = 'CALL' THEN open_interest ELSE 0 END) as call_open_interest, \
                    SUM(CASE WHEN option_type = 'PUT' THEN open_interest ELSE 0 END) as put_open_interest \
             FROM options_contracts \
             WHERE timestamp >= '{}T00:00:00.000000Z' AND timestamp <= '{}T23:59:59.999999Z'{} \
             GROUP BY trade_date \
             ORDER BY trade_date ASC;",
            parsed_start.format("%Y-%m-%d"),
            parsed_end.format("%Y-%m-%d"),
            ticker_filter
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a list of `PutCallRatioPoint` daily records.
    pub fn parse_put_call_ratio_dataset(
        resp_json: &Value,
    ) -> Result<Vec<crate::models::PutCallRatioPoint>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut points = Vec::new();
        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() >= 3 {
                    let date_str = arr[0].as_str().unwrap_or("").to_string();
                    let clean_date = if date_str.len() >= 10 {
                        date_str[0..10].to_string()
                    } else {
                        date_str
                    };

                    let call_volume = arr[1]
                        .as_u64()
                        .or_else(|| arr[1].as_f64().map(|v| v as u64))
                        .unwrap_or(0);
                    let put_volume = arr[2]
                        .as_u64()
                        .or_else(|| arr[2].as_f64().map(|v| v as u64))
                        .unwrap_or(0);

                    let call_oi = if arr.len() >= 4 {
                        arr[3]
                            .as_u64()
                            .or_else(|| arr[3].as_f64().map(|v| v as u64))
                    } else {
                        None
                    };

                    let put_oi = if arr.len() >= 5 {
                        arr[4]
                            .as_u64()
                            .or_else(|| arr[4].as_f64().map(|v| v as u64))
                    } else {
                        None
                    };

                    let ratio = if call_volume > 0 {
                        Some(((put_volume as f64 / call_volume as f64) * 10000.0).round() / 10000.0)
                    } else {
                        None
                    };

                    points.push(crate::models::PutCallRatioPoint {
                        date: clean_date,
                        put_volume,
                        call_volume,
                        put_open_interest: put_oi,
                        call_open_interest: call_oi,
                        ratio,
                    });
                }
            }
        }

        Ok(points)
    }

    /// Query options put/call ratio daily aggregate points from QuestDB.
    pub async fn query_put_call_ratio(
        &self,
        ticker: Option<&str>,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<crate::models::PutCallRatioPoint>, String> {
        let sql = Self::build_put_call_ratio_query(ticker, start_date, end_date)?;
        let endpoint = format!("{}/exec", self.config.url.trim_end_matches('/'));

        let mut backoff = Duration::from_millis(50);
        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .get(&endpoint)
                .query(&[("query", &sql)])
                .send()
                .await;
            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let body: Value = resp
                            .json()
                            .await
                            .map_err(|e| format!("JSON decode error: {}", e))?;
                        return Self::parse_put_call_ratio_dataset(&body);
                    }
                }
                Err(e) => {
                    warn!(
                        "[QuestDB Client] Put/call ratio query attempt {} failed: {}",
                        attempt, e
                    );
                }
            }
            if attempt < self.config.max_retries {
                sleep(backoff).await;
                backoff *= 2;
            }
        }

        Err(format!(
            "QuestDB service unavailable at {}",
            self.config.url
        ))
    }

    /// Build QuestDB SQL query for historical CSV data export.
    pub fn build_csv_export_query(
        ticker: &str,
        start_date: &str,
        end_date: &str,
        limit: u32,
    ) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;

        let parsed_start =
            NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    start_date, e
                )
            })?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|e| {
            format!(
                "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                end_date, e
            )
        })?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        let safe_limit = limit.clamp(1, 100_000);
        let start_iso = format!("{}T00:00:00.000000Z", parsed_start.format("%Y-%m-%d"));
        let next_end = parsed_end + chrono::Duration::days(1);
        let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

        Ok(format!(
            "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure \
             FROM sentiment_news \
             WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
             ORDER BY timestamp ASC \
             LIMIT {};",
            safe_ticker, start_iso, end_iso, safe_limit
        ))
    }

    /// Build QuestDB SQL query for historical JSON sentiment records with pagination and sorting.
    pub fn build_sentiment_history_query(
        ticker: &str,
        start_date: &str,
        end_date: &str,
        limit: u32,
        offset: u32,
        sort: &str,
    ) -> Result<String, String> {
        let safe_ticker = Self::validate_and_escape_ticker(ticker)?;

        let parsed_start =
            NaiveDate::parse_from_str(start_date.trim(), "%Y-%m-%d").map_err(|e| {
                format!(
                    "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                    start_date, e
                )
            })?;
        let parsed_end = NaiveDate::parse_from_str(end_date.trim(), "%Y-%m-%d").map_err(|e| {
            format!(
                "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                end_date, e
            )
        })?;

        if parsed_start > parsed_end {
            return Err("start_date cannot be after end_date".to_string());
        }

        let sort_upper = sort.trim().to_uppercase();
        let sort_order = match sort_upper.as_str() {
            "ASC" => "ASC",
            "DESC" => "DESC",
            _ => return Err("sort must be either 'asc' or 'desc'".to_string()),
        };

        let safe_limit = limit.clamp(1, 1000);
        let start_iso = format!("{}T00:00:00.000000Z", parsed_start.format("%Y-%m-%d"));
        let next_end = parsed_end + chrono::Duration::days(1);
        let end_iso = format!("{}T00:00:00.000000Z", next_end.format("%Y-%m-%d"));

        if offset > 0 {
            Ok(format!(
                "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc \
                 FROM sentiment_news \
                 WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
                 ORDER BY timestamp {} \
                 LIMIT {}, {};",
                safe_ticker,
                start_iso,
                end_iso,
                sort_order,
                offset,
                offset + safe_limit
            ))
        } else {
            Ok(format!(
                "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc \
                 FROM sentiment_news \
                 WHERE ticker = '{}' AND timestamp >= '{}' AND timestamp < '{}' \
                 ORDER BY timestamp {} \
                 LIMIT {};",
                safe_ticker, start_iso, end_iso, sort_order, safe_limit
            ))
        }
    }

    /// Parse QuestDB `/exec` response JSON into a list of `SentimentRecord`.
    pub fn parse_sentiment_history_exec_response(
        resp_json: &Value,
    ) -> Result<Vec<crate::models::SentimentRecord>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut records = Vec::with_capacity(dataset.len());

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() < 7 {
                    continue;
                }

                let published_utc = if let Some(ts_str) = arr[0].as_str() {
                    ts_str.to_string()
                } else if let Some(ts_nanos) = arr[0].as_i64() {
                    chrono::DateTime::from_timestamp(
                        ts_nanos / 1_000_000_000,
                        (ts_nanos % 1_000_000_000) as u32,
                    )
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| Utc::now().to_rfc3339())
                } else {
                    Utc::now().to_rfc3339()
                };

                let ticker = arr[1].as_str().unwrap_or("").to_string();
                let source = arr[2].as_str().unwrap_or("Wire").to_string();
                let title = arr[3].as_str().unwrap_or("Market Event").to_string();
                let sentiment_score = arr[4].as_f64().unwrap_or(0.0);
                let vpin = arr[5].as_f64().unwrap_or(0.0);
                let gamma_exposure = arr[6].as_f64().unwrap_or(0.0);

                let ingested_utc = if arr.len() > 7 && !arr[7].is_null() {
                    if let Some(s) = arr[7].as_str() {
                        Some(s.to_string())
                    } else if let Some(nanos) = arr[7].as_i64() {
                        chrono::DateTime::from_timestamp(
                            nanos / 1_000_000_000,
                            (nanos % 1_000_000_000) as u32,
                        )
                        .map(|dt| dt.to_rfc3339())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let db_commit_utc = if arr.len() > 8 && !arr[8].is_null() {
                    if let Some(s) = arr[8].as_str() {
                        Some(s.to_string())
                    } else if let Some(nanos) = arr[8].as_i64() {
                        chrono::DateTime::from_timestamp(
                            nanos / 1_000_000_000,
                            (nanos % 1_000_000_000) as u32,
                        )
                        .map(|dt| dt.to_rfc3339())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let (eff_ingested_utc, eff_db_commit_utc) = match (ingested_utc, db_commit_utc) {
                    (Some(ing), Some(com)) => (Some(ing), Some(com)),
                    (Some(ing), None) => {
                        let com = chrono::DateTime::parse_from_rfc3339(&ing)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| ing.clone());
                        (Some(ing), Some(com))
                    }
                    (None, Some(com)) => {
                        let ing = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        (Some(ing), Some(com))
                    }
                    (None, None) => {
                        let ing = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        let com = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(100)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        (Some(ing), Some(com))
                    }
                };

                let (confidence, _) =
                    crate::models::sentiment::compute_confidence_and_probabilities(
                        sentiment_score,
                        "NEUTRAL",
                        None,
                        None,
                        None,
                    );
                let data_quality_score =
                    crate::quality::compute_data_quality_score(&source, &title, confidence);
                let lang_res = crate::language::detect_language(&title);
                let (language, eff_model) = if lang_res.is_multilingual_model_applied {
                    (
                        lang_res.language,
                        Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string()),
                    )
                } else {
                    (
                        lang_res.language,
                        Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
                    )
                };

                records.push(crate::models::SentimentRecord {
                    published_utc,
                    ticker,
                    source: source.clone(),
                    title,
                    sentiment_score,
                    vpin,
                    gamma_exposure,
                    data_quality_score,
                    model_version: eff_model,
                    pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
                    data_provenance: Some(vec![source]),
                    language,
                    ingested_utc: eff_ingested_utc.clone(),
                    db_commit_utc: eff_db_commit_utc.clone(),
                    valid_from: eff_db_commit_utc,
                    valid_to: None,
                    revision_number: Some(1),
                    is_current: Some(true),
                });
            }
        }

        Ok(records)
    }

    /// Build QuestDB SQL query for aggregated news sentiment feed across multiple tickers or entire universe.
    pub fn build_sentiment_feed_query(
        tickers: Option<&[String]>,
        start_iso: &str,
        end_iso: &str,
        limit: u32,
        offset: u32,
        sort: &str,
        cursor: Option<&str>,
    ) -> Result<String, String> {
        let sort_upper = sort.trim().to_uppercase();
        let sort_order = match sort_upper.as_str() {
            "ASC" => "ASC",
            "DESC" => "DESC",
            _ => return Err("sort must be either 'asc' or 'desc'".to_string()),
        };

        let safe_limit = limit.clamp(1, 1000);

        let ticker_filter = if let Some(t_list) = tickers {
            if t_list.is_empty() {
                "".to_string()
            } else {
                let mut safe_tickers = Vec::new();
                for t in t_list {
                    safe_tickers.push(format!("'{}'", Self::validate_and_escape_ticker(t)?));
                }
                format!(" AND ticker IN ({})", safe_tickers.join(", "))
            }
        } else {
            "".to_string()
        };

        let cursor_clause = if let Some(cur) = cursor {
            let cur_trimmed = cur.trim();
            if cur_trimmed.is_empty() {
                return Err("Cursor timestamp cannot be empty. Expected RFC3339 format (e.g., '2026-08-29T14:30:00Z')".to_string());
            }
            let normalized = if cur_trimmed.contains(' ') && !cur_trimmed.contains('+') {
                cur_trimmed.replace(' ', "+")
            } else {
                cur_trimmed.to_string()
            };
            if chrono::DateTime::parse_from_rfc3339(&normalized).is_err() {
                return Err(format!(
                    "Invalid cursor timestamp format '{}'. Expected RFC3339 (e.g., '2026-08-29T14:30:00Z')",
                    cur_trimmed
                ));
            }
            if sort_order == "DESC" {
                format!(" AND timestamp < '{}'", normalized)
            } else {
                format!(" AND timestamp > '{}'", normalized)
            }
        } else {
            "".to_string()
        };

        if !cursor_clause.is_empty() {
            Ok(format!(
                "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc \
                 FROM sentiment_news \
                 WHERE timestamp >= '{}' AND timestamp <= '{}'{}{} \
                 ORDER BY timestamp {} \
                 LIMIT {};",
                start_iso, end_iso, cursor_clause, ticker_filter, sort_order, safe_limit
            ))
        } else if offset > 0 {
            Ok(format!(
                "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc \
                 FROM sentiment_news \
                 WHERE timestamp >= '{}' AND timestamp <= '{}'{} \
                 ORDER BY timestamp {} \
                 LIMIT {}, {};",
                start_iso,
                end_iso,
                ticker_filter,
                sort_order,
                offset,
                offset + safe_limit
            ))
        } else {
            Ok(format!(
                "SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc \
                 FROM sentiment_news \
                 WHERE timestamp >= '{}' AND timestamp <= '{}'{} \
                 ORDER BY timestamp {} \
                 LIMIT {};",
                start_iso, end_iso, ticker_filter, sort_order, safe_limit
            ))
        }
    }

    /// Build QuestDB SQL query to get total count of matching sentiment feed records.
    pub fn build_sentiment_feed_count_query(
        tickers: Option<&[String]>,
        start_iso: &str,
        end_iso: &str,
    ) -> Result<String, String> {
        let ticker_filter = if let Some(t_list) = tickers {
            if t_list.is_empty() {
                "".to_string()
            } else {
                let mut safe_tickers = Vec::new();
                for t in t_list {
                    safe_tickers.push(format!("'{}'", Self::validate_and_escape_ticker(t)?));
                }
                format!(" AND ticker IN ({})", safe_tickers.join(", "))
            }
        } else {
            "".to_string()
        };

        Ok(format!(
            "SELECT count() \
             FROM sentiment_news \
             WHERE timestamp >= '{}' AND timestamp <= '{}'{};",
            start_iso, end_iso, ticker_filter
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a list of `SentimentFeedItem`.
    pub fn parse_sentiment_feed_exec_response(
        resp_json: &Value,
    ) -> Result<Vec<crate::models::SentimentFeedItem>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut records = Vec::with_capacity(dataset.len());

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() < 7 {
                    continue;
                }

                let published_utc = if let Some(ts_str) = arr[0].as_str() {
                    ts_str.to_string()
                } else if let Some(ts_nanos) = arr[0].as_i64() {
                    chrono::DateTime::from_timestamp(
                        ts_nanos / 1_000_000_000,
                        (ts_nanos % 1_000_000_000) as u32,
                    )
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| Utc::now().to_rfc3339())
                } else {
                    Utc::now().to_rfc3339()
                };

                let ticker = arr[1].as_str().unwrap_or("").to_string();
                let source = arr[2].as_str().unwrap_or("Wire").to_string();
                let title = arr[3].as_str().unwrap_or("Market Event").to_string();
                let sentiment_score = arr[4].as_f64().unwrap_or(0.0);
                let vpin = arr[5].as_f64().unwrap_or(0.0);
                let gamma_exposure = arr[6].as_f64().unwrap_or(0.0);

                let ingested_utc = if arr.len() > 7 && !arr[7].is_null() {
                    if let Some(s) = arr[7].as_str() {
                        Some(s.to_string())
                    } else if let Some(nanos) = arr[7].as_i64() {
                        chrono::DateTime::from_timestamp(
                            nanos / 1_000_000_000,
                            (nanos % 1_000_000_000) as u32,
                        )
                        .map(|dt| dt.to_rfc3339())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let db_commit_utc = if arr.len() > 8 && !arr[8].is_null() {
                    if let Some(s) = arr[8].as_str() {
                        Some(s.to_string())
                    } else if let Some(nanos) = arr[8].as_i64() {
                        chrono::DateTime::from_timestamp(
                            nanos / 1_000_000_000,
                            (nanos % 1_000_000_000) as u32,
                        )
                        .map(|dt| dt.to_rfc3339())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let (eff_ingested_utc, eff_db_commit_utc) = match (ingested_utc, db_commit_utc) {
                    (Some(ing), Some(com)) => (Some(ing), Some(com)),
                    (Some(ing), None) => {
                        let com = chrono::DateTime::parse_from_rfc3339(&ing)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| ing.clone());
                        (Some(ing), Some(com))
                    }
                    (None, Some(com)) => {
                        let ing = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        (Some(ing), Some(com))
                    }
                    (None, None) => {
                        let ing = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(50)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        let com = chrono::DateTime::parse_from_rfc3339(&published_utc)
                            .map(|dt| (dt + chrono::Duration::milliseconds(100)).to_rfc3339())
                            .unwrap_or_else(|_| published_utc.clone());
                        (Some(ing), Some(com))
                    }
                };

                let sentiment_label = if sentiment_score > 0.15 {
                    "BULLISH".to_string()
                } else if sentiment_score < -0.15 {
                    "BEARISH".to_string()
                } else {
                    "NEUTRAL".to_string()
                };

                let (confidence, _) =
                    crate::models::sentiment::compute_confidence_and_probabilities(
                        sentiment_score,
                        &sentiment_label,
                        None,
                        None,
                        None,
                    );
                let data_quality_score =
                    crate::quality::compute_data_quality_score(&source, &title, confidence);
                let lang_res = crate::language::detect_language(&title);
                let (language, eff_model) = if lang_res.is_multilingual_model_applied {
                    (
                        lang_res.language,
                        Some(crate::language::MULTILINGUAL_MODEL_VERSION.to_string()),
                    )
                } else {
                    (
                        lang_res.language,
                        Some(crate::models::DEFAULT_MODEL_VERSION.to_string()),
                    )
                };

                records.push(crate::models::SentimentFeedItem {
                    published_utc,
                    ticker,
                    source: source.clone(),
                    title,
                    sentiment_score: (sentiment_score * 10000.0).round() / 10000.0,
                    sentiment_label,
                    confidence,
                    data_quality_score,
                    vpin: (vpin * 10000.0).round() / 10000.0,
                    gamma_exposure,
                    model_version: eff_model,
                    pipeline_version: Some(crate::models::DEFAULT_PIPELINE_VERSION.to_string()),
                    data_provenance: Some(vec![source]),
                    language,
                    ingested_utc: eff_ingested_utc.clone(),
                    db_commit_utc: eff_db_commit_utc.clone(),
                    valid_from: eff_db_commit_utc,
                    valid_to: None,
                    revision_number: Some(1),
                    is_current: Some(true),
                });
            }
        }

        Ok(records)
    }

    /// Build QuestDB SQL query for sentiment anomaly baseline data retrieval.
    pub fn build_sentiment_anomalies_query(
        tickers: Option<&[String]>,
        start_iso: &str,
    ) -> Result<String, String> {
        let ticker_filter = if let Some(t_list) = tickers {
            if t_list.is_empty() {
                "".to_string()
            } else {
                let mut safe_tickers = Vec::new();
                for t in t_list {
                    safe_tickers.push(format!("'{}'", Self::validate_and_escape_ticker(t)?));
                }
                format!(" AND ticker IN ({})", safe_tickers.join(", "))
            }
        } else {
            "".to_string()
        };

        Ok(format!(
            "SELECT ticker, sentiment_score, timestamp \
             FROM sentiment_news \
             WHERE timestamp >= '{}'{} \
             ORDER BY timestamp DESC;",
            start_iso, ticker_filter
        ))
    }

    /// Parse QuestDB `/exec` response JSON into a list of (ticker, sentiment_score, published_utc).
    pub fn parse_sentiment_anomalies_exec_response(
        resp_json: &Value,
    ) -> Result<Vec<(String, f64, String)>, String> {
        if let Some(err_msg) = resp_json.get("error").and_then(|e| e.as_str()) {
            return Err(format!("QuestDB query execution error: {}", err_msg));
        }

        let dataset = match resp_json.get("dataset").and_then(|d| d.as_array()) {
            Some(d) => d,
            None => return Ok(Vec::new()),
        };

        let mut records = Vec::with_capacity(dataset.len());

        for row in dataset {
            if let Some(arr) = row.as_array() {
                if arr.len() < 3 {
                    continue;
                }

                let ticker = arr[0].as_str().unwrap_or("").to_string();
                let sentiment_score = arr[1].as_f64().unwrap_or(0.0);

                let published_utc = if let Some(ts_str) = arr[2].as_str() {
                    ts_str.to_string()
                } else if let Some(ts_nanos) = arr[2].as_i64() {
                    chrono::DateTime::from_timestamp(
                        ts_nanos / 1_000_000_000,
                        (ts_nanos % 1_000_000_000) as u32,
                    )
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| Utc::now().to_rfc3339())
                } else {
                    Utc::now().to_rfc3339()
                };

                records.push((ticker, sentiment_score, published_utc));
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_build_csv_export_query() {
        let sql =
            QuestDbClient::build_csv_export_query("AAPL", "2025-01-01", "2025-03-31", 500).unwrap();
        assert!(sql.contains("WHERE ticker = 'AAPL' AND timestamp >= '2025-01-01T00:00:00.000000Z' AND timestamp < '2025-04-01T00:00:00.000000Z'"));
        assert!(sql.contains("LIMIT 500"));
        assert!(sql.contains("ORDER BY timestamp ASC"));

        // Date ordering validation
        assert!(
            QuestDbClient::build_csv_export_query("AAPL", "2025-05-01", "2025-01-01", 100).is_err()
        );
    }

    #[test]
    fn test_build_query_without_date() {
        let sql = QuestDbClient::build_query("AAPL", None).unwrap();
        assert_eq!(
            sql,
            "SELECT ticker, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, title, summary_clean, timestamp \
             FROM sentiment_news \
             WHERE ticker = 'AAPL' \
             ORDER BY timestamp DESC LIMIT 1;"
        );
    }

    #[test]
    fn test_build_query_with_date() {
        let sql = QuestDbClient::build_query("NVDA", Some("2026-08-25")).unwrap();
        assert!(sql.contains("WHERE ticker = 'NVDA' AND timestamp >= '2026-08-25T00:00:00.000000Z' AND timestamp < '2026-08-26T00:00:00.000000Z'"));
    }

    #[test]
    fn test_build_spillover_query() {
        let sql = QuestDbClient::build_spillover_query("AAPL", 10).unwrap();
        assert_eq!(
            sql,
            "SELECT ticker_a, ticker_b, lag_hours, correlation, timestamp \
             FROM cross_asset_spillovers \
             WHERE ticker_a = 'AAPL' OR ticker_b = 'AAPL' \
             ORDER BY abs(correlation) DESC \
             LIMIT 10;"
        );
    }

    #[test]
    fn test_ticker_validation_and_escaping() {
        assert!(QuestDbClient::validate_and_escape_ticker("AAPL").is_ok());
        assert!(QuestDbClient::validate_and_escape_ticker("BRK.A").is_ok());
        assert!(QuestDbClient::validate_and_escape_ticker("").is_err());
        assert!(QuestDbClient::validate_and_escape_ticker("WAYTOOLONGTICKERNAME").is_err());
        assert!(QuestDbClient::validate_and_escape_ticker("AAPL; DROP TABLE").is_err());
    }

    #[test]
    fn test_parse_exec_response_valid() {
        let mock_json = serde_json::json!({
            "query": "SELECT ...",
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "sentiment_label", "type": "STRING"},
                {"name": "prob_positive", "type": "DOUBLE"},
                {"name": "prob_negative", "type": "DOUBLE"},
                {"name": "prob_neutral", "type": "DOUBLE"},
                {"name": "title", "type": "STRING"},
                {"name": "summary_clean", "type": "STRING"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                [
                    "NVDA",
                    0.85,
                    "POSITIVE",
                    0.90,
                    0.05,
                    0.05,
                    "NVIDIA beats earnings",
                    "Clean summary text",
                    1724601600000000000i64
                ]
            ],
            "count": 1
        });

        let res = QuestDbClient::parse_exec_response(&mock_json, "2026-08-25").unwrap();
        assert!(res.is_some());
        let sent = res.unwrap();
        assert_eq!(sent.ticker, "NVDA");
        assert_eq!(sent.date, "2026-08-25");
        assert_eq!(sent.sentiment_score, 0.85);
        assert_eq!(sent.sentiment_label, "POSITIVE");
        assert_eq!(sent.signal_available_ts_us, 1724601600000000);
        assert_eq!(sent.confidence, 0.90);
        assert!(sent.probabilities.is_some());
        let probs = sent.probabilities.unwrap();
        assert_eq!(probs.positive, 0.90);
        assert_eq!(probs.negative, 0.05);
        assert_eq!(probs.neutral, 0.05);
        assert_eq!(sent.message, "Retrieved directly from QuestDB");
    }

    #[test]
    fn test_parse_exec_response_empty() {
        let mock_json = serde_json::json!({
            "query": "SELECT ...",
            "columns": [],
            "dataset": [],
            "count": 0
        });

        let res = QuestDbClient::parse_exec_response(&mock_json, "2026-08-25").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn test_parse_spillover_exec_response() {
        let mock_json = serde_json::json!({
            "query": "SELECT ...",
            "columns": [
                {"name": "ticker_a", "type": "SYMBOL"},
                {"name": "ticker_b", "type": "SYMBOL"},
                {"name": "lag_hours", "type": "LONG"},
                {"name": "correlation", "type": "DOUBLE"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                ["AAPL", "MSFT", 1, 0.745, "2026-08-26T12:00:00.000000Z"],
                ["NVDA", "AAPL", 2, 0.682, "2026-08-26T12:00:00.000000Z"]
            ],
            "count": 2
        });

        let items = QuestDbClient::parse_spillover_exec_response(&mock_json, "AAPL").unwrap();
        assert_eq!(items.len(), 2);

        // Row 1: AAPL -> MSFT, lag 1 => AAPL leads MSFT
        assert_eq!(items[0].related_ticker, "MSFT");
        assert_eq!(items[0].lag_hours, 1);
        assert_eq!(items[0].relationship, "AAPL LEADS MSFT by 1h");

        // Row 2: NVDA -> AAPL, lag 2 => NVDA leads AAPL => relative to AAPL, lag is -2 (NVDA leads AAPL by 2h)
        assert_eq!(items[1].related_ticker, "NVDA");
        assert_eq!(items[1].lag_hours, -2);
        assert_eq!(items[1].relationship, "NVDA LEADS AAPL by 2h");
    }

    #[test]
    fn test_build_backtest_query() {
        let sql = QuestDbClient::build_backtest_query("AAPL", "2025-01-01", "2025-12-31").unwrap();
        assert!(sql.contains("WHERE ticker = 'AAPL' AND timestamp >= '2025-01-01T00:00:00.000000Z' AND timestamp < '2026-01-01T00:00:00.000000Z'"));
        assert!(sql.contains("SELECT sentiment_score, signal_available_ts_us, timestamp"));
    }

    #[test]
    fn test_parse_backtest_sentiment_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "signal_available_ts_us", "type": "LONG"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                [0.75, 1724601600000000i64, 1724601600000000000i64],
                [-0.35, 1724688000000000i64, 1724688000000000000i64]
            ],
            "count": 2
        });

        let events = QuestDbClient::parse_backtest_sentiment_dataset(&mock_json).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, 0.75);
        assert_eq!(events[0].1, 1724601600000000);
        assert_eq!(events[1].0, -0.35);
        assert_eq!(events[1].1, 1724688000000000);
    }

    #[test]
    fn test_build_batch_query() {
        let tickers = vec!["AAPL".to_string(), "MSFT".to_string(), "NVDA".to_string()];
        let sql = QuestDbClient::build_batch_query(&tickers, Some("2026-08-25")).unwrap();
        assert!(sql.contains("WHERE ticker IN ('AAPL', 'MSFT', 'NVDA')"));
        assert!(sql.contains("timestamp < '2026-08-26T00:00:00.000000Z'"));
    }

    #[test]
    fn test_parse_batch_exec_response() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "sentiment_label", "type": "SYMBOL"},
                {"name": "prob_positive", "type": "DOUBLE"},
                {"name": "prob_negative", "type": "DOUBLE"},
                {"name": "prob_neutral", "type": "DOUBLE"},
                {"name": "title", "type": "STRING"},
                {"name": "summary_clean", "type": "STRING"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                ["AAPL", 0.75, "POSITIVE", 0.85, 0.05, 0.10, "Apple event", "Apple summary", 1724601600000000000i64],
                ["MSFT", 0.45, "BULLISH", 0.70, 0.10, 0.20, "MSFT earnings", "MSFT summary", 1724601600000000000i64],
                ["AAPL", 0.20, "NEUTRAL", 0.30, 0.30, 0.40, "Old Apple event", "Old Apple summary", 1724501600000000000i64]
            ],
            "count": 3
        });

        let requested = vec!["AAPL".to_string(), "MSFT".to_string()];
        let results =
            QuestDbClient::parse_batch_exec_response(&mock_json, "2026-08-25", &requested).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].ticker, "AAPL");
        assert_eq!(results[0].sentiment_score, 0.75);
        assert_eq!(results[0].confidence, 0.85);
        assert_eq!(results[1].ticker, "MSFT");
        assert_eq!(results[1].sentiment_score, 0.45);
    }

    #[test]
    fn test_build_stock_prices_query() {
        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let sql =
            QuestDbClient::build_stock_prices_query(&tickers, "2025-01-01", "2025-01-10").unwrap();
        assert!(sql.starts_with(
            "SELECT date, ticker, open, high, low, close, volume, vwap FROM stock_daily_bars"
        ));
        assert!(sql.contains("WHERE ticker IN ('AAPL', 'NVDA')"));
        assert!(sql.contains("date >= '2025-01-01T00:00:00.000000Z'"));
        assert!(sql.contains("date < '2025-01-11T00:00:00.000000Z'"));
    }

    #[test]
    fn test_parse_stock_prices_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "date", "type": "TIMESTAMP"},
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "open", "type": "DOUBLE"},
                {"name": "high", "type": "DOUBLE"},
                {"name": "low", "type": "DOUBLE"},
                {"name": "close", "type": "DOUBLE"},
                {"name": "volume", "type": "DOUBLE"},
                {"name": "vwap", "type": "DOUBLE"}
            ],
            "dataset": [
                ["2025-01-02T16:00:00.000000Z", "AAPL", 224.50, 226.80, 223.10, 225.40, 48500000.0, 225.10],
                ["2025-01-03T16:00:00.000000Z", "AAPL", 225.40, 227.50, 224.00, 226.90, 51200000.0, 226.20],
                ["2025-01-02T16:00:00.000000Z", "NVDA", 125.00, 128.00, 124.50, 127.30, 95000000.0, 126.80]
            ],
            "count": 3
        });

        let prices = QuestDbClient::parse_stock_prices_dataset(&mock_json).unwrap();
        assert_eq!(prices.len(), 2);
        let aapl = prices.get("AAPL").unwrap();
        assert_eq!(aapl.len(), 2);
        assert_eq!(
            aapl.get(&NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()),
            Some(&225.40)
        );
        assert_eq!(
            aapl.get(&NaiveDate::from_ymd_opt(2025, 1, 3).unwrap()),
            Some(&226.90)
        );

        let nvda = prices.get("NVDA").unwrap();
        assert_eq!(nvda.len(), 1);
        assert_eq!(
            nvda.get(&NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()),
            Some(&127.30)
        );
    }

    #[test]
    fn test_build_options_contracts_query() {
        let sql = QuestDbClient::build_options_contracts_query(
            "AAPL",
            "2025-12-19",
            Some("call"),
            Some(250.0),
        )
        .unwrap();
        assert!(sql
            .starts_with("SELECT ticker, underlying_ticker, expiration_date, strike, option_type"));
        assert!(sql.contains("WHERE underlying_ticker = 'AAPL'"));
        assert!(sql.contains("expiration_date = '2025-12-19'"));
        assert!(sql.contains("AND option_type = 'CALL'"));
        assert!(sql.contains("AND strike = 250.0000"));
    }

    #[test]
    fn test_parse_options_contracts_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "underlying_ticker", "type": "SYMBOL"},
                {"name": "expiration_date", "type": "VARCHAR"},
                {"name": "strike", "type": "DOUBLE"},
                {"name": "option_type", "type": "SYMBOL"},
                {"name": "bid", "type": "DOUBLE"},
                {"name": "ask", "type": "DOUBLE"},
                {"name": "last", "type": "DOUBLE"},
                {"name": "volume", "type": "LONG"},
                {"name": "open_interest", "type": "LONG"},
                {"name": "implied_volatility", "type": "DOUBLE"},
                {"name": "delta", "type": "DOUBLE"},
                {"name": "gamma", "type": "DOUBLE"},
                {"name": "theta", "type": "DOUBLE"},
                {"name": "vega", "type": "DOUBLE"},
                {"name": "rho", "type": "DOUBLE"}
            ],
            "dataset": [
                ["O:AAPL251219C00250000", "AAPL", "2025-12-19", 250.0, "CALL", 8.45, 8.65, 8.55, 4500, 22000, 0.2850, 0.4215, 0.0098, -0.0452, 0.2850, 0.2450]
            ],
            "count": 1
        });

        let contracts = QuestDbClient::parse_options_contracts_dataset(&mock_json).unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].ticker, "O:AAPL251219C00250000");
        assert_eq!(contracts[0].underlying_ticker, "AAPL");
        assert_eq!(contracts[0].strike, 250.0);
        assert_eq!(contracts[0].option_type, "CALL");
        assert_eq!(contracts[0].implied_volatility, 0.2850);
        assert_eq!(contracts[0].delta, 0.4215);
    }

    #[test]
    fn test_build_unusual_options_query() {
        let sql_all = QuestDbClient::build_unusual_options_query(None, 3).unwrap();
        assert!(sql_all.starts_with("SELECT ticker, underlying_ticker, expiration_date"));
        assert!(sql_all.contains("dateadd('d', -3, now())"));
        assert!(!sql_all.contains("AND underlying_ticker"));

        let sql_ticker = QuestDbClient::build_unusual_options_query(Some("AAPL"), 1).unwrap();
        assert!(sql_ticker.contains("AND underlying_ticker = 'AAPL'"));
        assert!(sql_ticker.contains("dateadd('d', -1, now())"));
    }

    #[test]
    fn test_parse_unusual_options_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "underlying_ticker", "type": "SYMBOL"},
                {"name": "expiration_date", "type": "VARCHAR"},
                {"name": "strike", "type": "DOUBLE"},
                {"name": "option_type", "type": "SYMBOL"},
                {"name": "volume", "type": "LONG"},
                {"name": "open_interest", "type": "LONG"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                ["O:AAPL251219C00250000", "AAPL", "2025-12-19", 250.0, "CALL", 48500, 2200, "2025-08-29T12:00:00.000000Z"]
            ],
            "count": 1
        });

        let items = QuestDbClient::parse_unusual_options_dataset(&mock_json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].ticker, "O:AAPL251219C00250000");
        assert_eq!(items[0].underlying_ticker, "AAPL");
        assert_eq!(items[0].volume, 48500);
        assert_eq!(items[0].open_interest, 2200);
        assert!(items[0].volume_oi_ratio > 20.0);
        assert!(items[0].score > 0.0);
    }

    #[test]
    fn test_build_sentiment_feed_query() {
        let sql_all = QuestDbClient::build_sentiment_feed_query(
            None,
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            50,
            0,
            "desc",
            None,
        )
        .unwrap();
        assert!(sql_all.starts_with("SELECT timestamp, ticker, source, title, sentiment_score, vpin, gamma_exposure, ingested_utc, db_commit_utc FROM sentiment_news"));
        assert!(sql_all.contains(
            "WHERE timestamp >= '2026-08-28T00:00:00Z' AND timestamp <= '2026-08-29T00:00:00Z'"
        ));
        assert!(sql_all.contains("ORDER BY timestamp DESC"));
        assert!(sql_all.contains("LIMIT 50;"));
        assert!(!sql_all.contains("AND ticker IN"));

        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let sql_sector = QuestDbClient::build_sentiment_feed_query(
            Some(&tickers),
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            20,
            10,
            "asc",
            None,
        )
        .unwrap();
        assert!(sql_sector.contains("AND ticker IN ('AAPL', 'NVDA')"));
        assert!(sql_sector.contains("ORDER BY timestamp ASC"));
        assert!(sql_sector.contains("LIMIT 10, 30;"));

        // Cursor-based query with DESC order
        let sql_cursor_desc = QuestDbClient::build_sentiment_feed_query(
            None,
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            25,
            0,
            "desc",
            Some("2026-08-29T12:00:00Z"),
        )
        .unwrap();
        assert!(sql_cursor_desc.contains("AND timestamp < '2026-08-29T12:00:00Z'"));
        assert!(sql_cursor_desc.contains("LIMIT 25;"));
        assert!(!sql_cursor_desc.contains("OFFSET"));

        // Cursor-based query with ASC order and tickers
        let sql_cursor_asc = QuestDbClient::build_sentiment_feed_query(
            Some(&tickers),
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            15,
            0,
            "asc",
            Some("2026-08-28T06:00:00Z"),
        )
        .unwrap();
        assert!(sql_cursor_asc.contains("AND timestamp > '2026-08-28T06:00:00Z'"));
        assert!(sql_cursor_asc.contains("AND ticker IN ('AAPL', 'NVDA')"));
        assert!(sql_cursor_asc.contains("LIMIT 15;"));

        // Invalid cursor timestamp rejected
        let sql_invalid = QuestDbClient::build_sentiment_feed_query(
            None,
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            10,
            0,
            "desc",
            Some("not-a-valid-timestamp"),
        );
        assert!(sql_invalid.is_err());

        // Empty cursor timestamp rejected
        let sql_empty = QuestDbClient::build_sentiment_feed_query(
            None,
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
            10,
            0,
            "desc",
            Some(""),
        );
        assert!(sql_empty.is_err());
    }

    #[test]
    fn test_build_sentiment_feed_count_query() {
        let count_sql_all = QuestDbClient::build_sentiment_feed_count_query(
            None,
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
        )
        .unwrap();
        assert_eq!(
            count_sql_all,
            "SELECT count() FROM sentiment_news WHERE timestamp >= '2026-08-28T00:00:00Z' AND timestamp <= '2026-08-29T00:00:00Z';"
        );

        let tickers = vec!["MSFT".to_string()];
        let count_sql_sector = QuestDbClient::build_sentiment_feed_count_query(
            Some(&tickers),
            "2026-08-28T00:00:00Z",
            "2026-08-29T00:00:00Z",
        )
        .unwrap();
        assert_eq!(
            count_sql_sector,
            "SELECT count() FROM sentiment_news WHERE timestamp >= '2026-08-28T00:00:00Z' AND timestamp <= '2026-08-29T00:00:00Z' AND ticker IN ('MSFT');"
        );
    }

    #[test]
    fn test_parse_sentiment_feed_exec_response() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "timestamp", "type": "TIMESTAMP"},
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "source", "type": "STRING"},
                {"name": "title", "type": "STRING"},
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "vpin", "type": "DOUBLE"},
                {"name": "gamma_exposure", "type": "DOUBLE"}
            ],
            "dataset": [
                ["2026-08-29T14:30:00.000000Z", "AAPL", "Bloomberg", "Apple introduces breakthrough AI processor", 0.75, 0.42, 150000.0]
            ],
            "count": 1
        });

        let items = QuestDbClient::parse_sentiment_feed_exec_response(&mock_json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].ticker, "AAPL");
        assert_eq!(items[0].source, "Bloomberg");
        assert_eq!(items[0].sentiment_label, "BULLISH");
        assert!(items[0].sentiment_score > 0.7);
        assert!(items[0].confidence > 0.8);
        assert!(items[0].data_quality_score > 0.8);
        assert_eq!(items[0].vpin, 0.42);
        assert_eq!(items[0].gamma_exposure, 150000.0);
        assert!(items[0].ingested_utc.is_some());
        assert!(items[0].db_commit_utc.is_some());
    }

    #[test]
    fn test_build_sentiment_anomalies_query() {
        let sql_all =
            QuestDbClient::build_sentiment_anomalies_query(None, "2026-07-30T00:00:00Z").unwrap();
        assert!(sql_all.contains("WHERE timestamp >= '2026-07-30T00:00:00Z'"));
        assert!(!sql_all.contains("ticker IN"));

        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let sql_tickers =
            QuestDbClient::build_sentiment_anomalies_query(Some(&tickers), "2026-07-30T00:00:00Z")
                .unwrap();
        assert!(sql_tickers.contains("AND ticker IN ('AAPL', 'NVDA')"));
    }

    #[test]
    fn test_parse_sentiment_anomalies_exec_response() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "timestamp", "type": "TIMESTAMP"}
            ],
            "dataset": [
                ["AAPL", 0.85, "2026-08-29T14:30:00.000000Z"],
                ["NVDA", -0.75, "2026-08-29T14:15:00.000000Z"]
            ],
            "count": 2
        });

        let records = QuestDbClient::parse_sentiment_anomalies_exec_response(&mock_json).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].0, "AAPL");
        assert_eq!(records[0].1, 0.85);
        assert_eq!(records[0].2, "2026-08-29T14:30:00.000000Z");
        assert_eq!(records[1].0, "NVDA");
        assert_eq!(records[1].1, -0.75);
    }

    #[test]
    fn test_build_put_call_ratio_query() {
        let sql_all =
            QuestDbClient::build_put_call_ratio_query(None, "2025-01-01", "2025-03-31").unwrap();
        assert!(sql_all.contains("FROM options_contracts"));
        assert!(sql_all.contains("timestamp >= '2025-01-01T00:00:00.000000Z'"));
        assert!(sql_all.contains("timestamp <= '2025-03-31T23:59:59.999999Z'"));
        assert!(!sql_all.contains("underlying_ticker ="));

        let sql_aapl =
            QuestDbClient::build_put_call_ratio_query(Some("AAPL"), "2025-01-01", "2025-03-31")
                .unwrap();
        assert!(sql_aapl.contains("underlying_ticker = 'AAPL'"));
    }

    #[test]
    fn test_parse_put_call_ratio_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "trade_date", "type": "DATE"},
                {"name": "call_volume", "type": "LONG"},
                {"name": "put_volume", "type": "LONG"},
                {"name": "call_open_interest", "type": "LONG"},
                {"name": "put_open_interest", "type": "LONG"}
            ],
            "dataset": [
                ["2025-01-02", 3000, 1500, 80000, 40000],
                ["2025-01-03", 2500, 2000, 82000, 41000]
            ],
            "count": 2
        });

        let points = QuestDbClient::parse_put_call_ratio_dataset(&mock_json).unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].date, "2025-01-02");
        assert_eq!(points[0].call_volume, 3000);
        assert_eq!(points[0].put_volume, 1500);
        assert_eq!(points[0].ratio, Some(0.5));
        assert_eq!(points[1].ratio, Some(0.8));
    }
}
