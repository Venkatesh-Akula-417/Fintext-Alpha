//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — QuestDB HTTP SQL Reader & Influx Line Protocol (ILP) Writer
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::correlation::SpilloverResult;
use crate::timeseries::{SentimentEvent, MICROS_PER_HOUR};
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

/// Escape tag values for Influx Line Protocol (ILP)
pub fn escape_tag_value(v: &str) -> String {
    v.replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
        .replace(['\n', '\r'], "")
}

/// Formats a list of `SpilloverResult` items into ILP payload lines.
pub fn format_spillovers_ilp(spillovers: &[SpilloverResult], timestamp_nanos: i64) -> String {
    let mut lines = Vec::with_capacity(spillovers.len());
    for s in spillovers {
        let tag_a = escape_tag_value(&s.ticker_a);
        let tag_b = escape_tag_value(&s.ticker_b);
        lines.push(format!(
            "cross_asset_spillovers,ticker_a={},ticker_b={} lag_hours={}i,correlation={:.4},num_obs={}i {}",
            tag_a, tag_b, s.lag_hours, s.correlation, s.num_observations, timestamp_nanos
        ));
    }
    lines.join("\n")
}

/// Fetch historical sentiment events from QuestDB via HTTP SQL REST endpoint `/exec`.
pub async fn fetch_sentiment_history(
    client: &Client,
    base_url: &str,
    lookback_days: i64,
    mock_mode: bool,
) -> Result<Vec<SentimentEvent>, String> {
    if mock_mode {
        info!(
            "[QuestDB Client] Mock mode active: Generating synthetic multi-asset sentiment history"
        );
        return Ok(generate_mock_sentiment_events(lookback_days));
    }

    let endpoint = format!("{}/exec", base_url.trim_end_matches('/'));

    // Primary SQL with dateadd filter
    let sql = format!(
        "SELECT ticker, sentiment_score, signal_available_ts_us, timestamp \
         FROM sentiment_news \
         WHERE timestamp >= dateadd('d', -{}, now()) \
         ORDER BY timestamp ASC;",
        lookback_days
    );

    let resp_res = client.get(&endpoint).query(&[("query", &sql)]).send().await;

    let resp = match resp_res {
        Ok(r) => r,
        Err(e) => {
            // Fallback query if table exists without designated timestamp filter
            let fallback_sql = "SELECT ticker, sentiment_score, signal_available_ts_us FROM sentiment_news ORDER BY signal_available_ts_us ASC;";
            let fallback_res = client
                .get(&endpoint)
                .query(&[("query", fallback_sql)])
                .send()
                .await;
            match fallback_res {
                Ok(fr) => fr,
                Err(_) => {
                    return Err(format!(
                        "Failed to connect to QuestDB at {}: {}",
                        endpoint, e
                    ))
                }
            }
        }
    };

    if !resp.status().is_success() {
        return Err(format!("QuestDB returned HTTP status: {}", resp.status()));
    }

    let body_json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse QuestDB JSON response: {}", e))?;

    parse_sentiment_dataset(&body_json)
}

/// Parse QuestDB `/exec` response JSON into a `Vec<SentimentEvent>`.
pub fn parse_sentiment_dataset(json: &Value) -> Result<Vec<SentimentEvent>, String> {
    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
        return Err(format!("QuestDB query error: {}", err));
    }

    let columns = json.get("columns").and_then(|c| c.as_array());
    let dataset = json.get("dataset").and_then(|d| d.as_array());

    let (cols, rows) = match (columns, dataset) {
        (Some(c), Some(r)) => (c, r),
        _ => return Ok(Vec::new()),
    };

    let mut idx_ticker = None;
    let mut idx_score = None;
    let mut idx_ts = None;

    for (i, col) in cols.iter().enumerate() {
        if let Some(name) = col.get("name").and_then(|n| n.as_str()) {
            match name {
                "ticker" => idx_ticker = Some(i),
                "sentiment_score" => idx_score = Some(i),
                "signal_available_ts_us" | "timestamp"
                    if idx_ts.is_none() || name == "signal_available_ts_us" =>
                {
                    idx_ts = Some(i);
                }
                _ => {}
            }
        }
    }

    let idx_ticker =
        idx_ticker.ok_or_else(|| "Missing 'ticker' column in QuestDB dataset".to_string())?;
    let idx_score = idx_score
        .ok_or_else(|| "Missing 'sentiment_score' column in QuestDB dataset".to_string())?;
    let idx_ts = idx_ts.unwrap_or(2);

    let mut events = Vec::with_capacity(rows.len());

    for row in rows {
        if let Some(arr) = row.as_array() {
            if arr.len() <= idx_ticker || arr.len() <= idx_score {
                continue;
            }

            let ticker = match arr[idx_ticker].as_str() {
                Some(t) => t.to_string(),
                None => continue,
            };

            let score = match arr[idx_score].as_f64() {
                Some(s) => s,
                None => continue,
            };

            let ts_us = if arr.len() > idx_ts {
                if let Some(us) = arr[idx_ts].as_i64() {
                    if us > 1_000_000_000_000_000_000 {
                        // Nanoseconds -> Microseconds
                        us / 1_000
                    } else {
                        us
                    }
                } else if let Some(ts_str) = arr[idx_ts].as_str() {
                    chrono::DateTime::parse_from_rfc3339(ts_str)
                        .map(|dt| dt.timestamp_micros())
                        .unwrap_or_else(|_| Utc::now().timestamp_micros())
                } else {
                    Utc::now().timestamp_micros()
                }
            } else {
                Utc::now().timestamp_micros()
            };

            events.push(SentimentEvent {
                ticker,
                sentiment_score: score,
                timestamp_us: ts_us,
            });
        }
    }

    Ok(events)
}

/// Ingest calculated spillover relationships into QuestDB using HTTP Influx Line Protocol (ILP).
pub async fn write_spillovers_ilp(
    client: &Client,
    base_url: &str,
    spillovers: &[SpilloverResult],
    max_retries: usize,
    mock_mode: bool,
) -> Result<usize, String> {
    if spillovers.is_empty() {
        return Ok(0);
    }

    if mock_mode {
        info!(
            "[QuestDB Sink] Mock mode active: Successfully committed {} spillovers to virtual QuestDB table",
            spillovers.len()
        );
        return Ok(spillovers.len());
    }

    let timestamp_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let ilp_payload = format_spillovers_ilp(spillovers, timestamp_nanos);
    let endpoint = format!("{}/write?precision=n", base_url.trim_end_matches('/'));

    let mut backoff = Duration::from_millis(100);

    for attempt in 1..=max_retries {
        let resp_res = client
            .post(&endpoint)
            .body(ilp_payload.clone())
            .header("Content-Type", "text/plain; charset=utf-8")
            .send()
            .await;

        match resp_res {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!(
                        "[QuestDB ILP] Ingested {} cross-asset spillover records into table 'cross_asset_spillovers'",
                        spillovers.len()
                    );
                    return Ok(spillovers.len());
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    warn!(
                        "[QuestDB ILP] Attempt {}/{} failed with status: {} (Detail: {})",
                        attempt, max_retries, err_text, err_text
                    );
                }
            }
            Err(e) => {
                warn!(
                    "[QuestDB ILP] Attempt {}/{} connection error: {}",
                    attempt, max_retries, e
                );
            }
        }

        if attempt < max_retries {
            sleep(backoff).await;
            backoff *= 2;
        }
    }

    Err(format!(
        "Failed to write spillovers to QuestDB at {} after {} retries",
        endpoint, max_retries
    ))
}

/// Generates representative synthetic historical sentiment events for testing and simulation.
pub fn generate_mock_sentiment_events(lookback_days: i64) -> Vec<SentimentEvent> {
    let now_us = Utc::now().timestamp_micros();
    let total_hours = (lookback_days * 24).min(720) as usize;
    let start_us = now_us - (total_hours as i64 * MICROS_PER_HOUR);

    let mut events = Vec::with_capacity(total_hours * 5);

    // Synthetic lead-lag pattern:
    // NVDA leads AAPL by 1 hour with positive correlation
    // MSFT is correlated with AAPL contemporaneously
    for h in 0..total_hours {
        let ts = start_us + (h as i64 * MICROS_PER_HOUR);
        let base_signal = ((h as f64 * 0.1).sin() * 0.7) + ((h as f64 * 0.05).cos() * 0.3);

        // NVDA: base signal
        events.push(SentimentEvent {
            ticker: "NVDA".to_string(),
            sentiment_score: (base_signal + 0.1).clamp(-1.0, 1.0),
            timestamp_us: ts,
        });

        // AAPL: follows NVDA 1 hour later
        if h >= 1 {
            let prev_signal =
                (((h - 1) as f64 * 0.1).sin() * 0.7) + (((h - 1) as f64 * 0.05).cos() * 0.3);
            events.push(SentimentEvent {
                ticker: "AAPL".to_string(),
                sentiment_score: (prev_signal * 0.85 + 0.05).clamp(-1.0, 1.0),
                timestamp_us: ts,
            });
        }

        // MSFT: contemporaneous with AAPL
        events.push(SentimentEvent {
            ticker: "MSFT".to_string(),
            sentiment_score: (base_signal * 0.75 - 0.05).clamp(-1.0, 1.0),
            timestamp_us: ts,
        });

        // GOOGL: independent signal
        let googl_signal = ((h as f64 * 0.2).cos() * 0.6).clamp(-1.0, 1.0);
        events.push(SentimentEvent {
            ticker: "GOOGL".to_string(),
            sentiment_score: googl_signal,
            timestamp_us: ts,
        });
    }

    events
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_format_spillovers_ilp() {
        let spillovers = vec![
            SpilloverResult {
                ticker_a: "AAPL".to_string(),
                ticker_b: "MSFT".to_string(),
                lag_hours: 1,
                correlation: 0.7654,
                num_observations: 240,
            },
            SpilloverResult {
                ticker_a: "NVDA".to_string(),
                ticker_b: "AAPL".to_string(),
                lag_hours: -2,
                correlation: -0.4321,
                num_observations: 180,
            },
        ];

        let ilp = format_spillovers_ilp(&spillovers, 1724601600000000000);
        assert!(ilp.contains("cross_asset_spillovers,ticker_a=AAPL,ticker_b=MSFT lag_hours=1i,correlation=0.7654,num_obs=240i 1724601600000000000"));
        assert!(ilp.contains("cross_asset_spillovers,ticker_a=NVDA,ticker_b=AAPL lag_hours=-2i,correlation=-0.4321,num_obs=180i 1724601600000000000"));
    }

    #[test]
    fn test_parse_sentiment_dataset() {
        let mock_json = serde_json::json!({
            "columns": [
                {"name": "ticker", "type": "SYMBOL"},
                {"name": "sentiment_score", "type": "DOUBLE"},
                {"name": "signal_available_ts_us", "type": "LONG"}
            ],
            "dataset": [
                ["AAPL", 0.82, 1724601600000000i64],
                ["NVDA", -0.45, 1724601600000000i64]
            ],
            "count": 2
        });

        let events = parse_sentiment_dataset(&mock_json).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].ticker, "AAPL");
        assert_eq!(events[0].sentiment_score, 0.82);
        assert_eq!(events[0].timestamp_us, 1724601600000000);
    }
}
