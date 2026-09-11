//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Time Series Aggregation & Hourly Bucketing
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Microseconds in a single standard hour (3,600 seconds * 1,000,000 µs)
pub const MICROS_PER_HOUR: i64 = 3_600_000_000;

/// Raw incoming sentiment record extracted from QuestDB `sentiment_news`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SentimentEvent {
    pub ticker: String,
    pub sentiment_score: f64,
    pub timestamp_us: i64,
}

/// Convert microsecond timestamp to discrete integer epoch hour index.
#[inline]
pub fn timestamp_to_epoch_hour(timestamp_us: i64) -> i64 {
    timestamp_us / MICROS_PER_HOUR
}

/// Convert epoch hour index back to start timestamp in microseconds.
#[inline]
pub fn epoch_hour_to_timestamp(epoch_hour: i64) -> i64 {
    epoch_hour * MICROS_PER_HOUR
}

/// Aggregates raw sentiment events into dense, synchronized hourly time series per ticker.
///
/// Steps:
/// 1. Maps each event into its corresponding epoch hour index.
/// 2. Computes the arithmetic mean sentiment for each `(ticker, epoch_hour)`.
/// 3. Filters out tickers with fewer than `min_active_hours` discrete observations.
/// 4. Caps total tickers to `max_tickers` (ranked by activity) to ensure strictly bounded RAM (<2GB).
/// 5. Fills missing time intervals with 0.0 (neutral sentiment baseline) to produce uniform vectors.
pub fn build_synchronized_matrix(
    events: &[SentimentEvent],
    min_active_hours: usize,
    max_tickers: usize,
) -> (Vec<i64>, HashMap<String, Vec<f64>>) {
    if events.is_empty() {
        return (Vec::new(), HashMap::new());
    }

    // 1. Group by (ticker, epoch_hour) -> (sum_score, count)
    let mut ticker_hourly: HashMap<String, HashMap<i64, (f64, usize)>> = HashMap::new();
    let mut min_hour = i64::MAX;
    let mut max_hour = i64::MIN;

    for ev in events {
        let ticker = ev.ticker.trim().to_uppercase();
        if ticker.is_empty() || ticker == "MARKET" || ticker == "UNKNOWN" {
            continue;
        }

        let hour = timestamp_to_epoch_hour(ev.timestamp_us);
        if hour < min_hour {
            min_hour = hour;
        }
        if hour > max_hour {
            max_hour = hour;
        }

        let entry = ticker_hourly
            .entry(ticker)
            .or_default()
            .entry(hour)
            .or_insert((0.0, 0));
        entry.0 += ev.sentiment_score;
        entry.1 += 1;
    }

    if min_hour > max_hour {
        return (Vec::new(), HashMap::new());
    }

    // 2. Filter tickers by activity threshold
    let mut qualifying_tickers: Vec<(String, usize)> = ticker_hourly
        .iter()
        .map(|(t, map)| (t.clone(), map.len()))
        .filter(|(_, count)| *count >= min_active_hours)
        .collect();

    // Sort by count descending (most active first) and limit to max_tickers
    qualifying_tickers.sort_by_key(|b| std::cmp::Reverse(b.1));
    if qualifying_tickers.len() > max_tickers {
        qualifying_tickers.truncate(max_tickers);
    }

    if qualifying_tickers.is_empty() {
        return (Vec::new(), HashMap::new());
    }

    // 3. Build uniform contiguous timeline [min_hour..=max_hour]
    let total_hours = (max_hour - min_hour + 1) as usize;
    let mut timeline: Vec<i64> = Vec::with_capacity(total_hours);
    for h in min_hour..=max_hour {
        timeline.push(h);
    }

    // 4. Populate aligned series vector per qualifying ticker
    let mut aligned_series: HashMap<String, Vec<f64>> =
        HashMap::with_capacity(qualifying_tickers.len());

    for (ticker, _) in qualifying_tickers {
        let hourly_map = &ticker_hourly[&ticker];
        let mut series = Vec::with_capacity(total_hours);

        for &h in &timeline {
            if let Some(&(sum, cnt)) = hourly_map.get(&h) {
                if cnt > 0 {
                    series.push(sum / cnt as f64);
                } else {
                    series.push(0.0);
                }
            } else {
                series.push(0.0);
            }
        }

        aligned_series.insert(ticker, series);
    }

    (timeline, aligned_series)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_epoch_hour_conversion() {
        let t0 = 1724601600000000_i64; // 2024-08-25T16:00:00Z
        let hour = timestamp_to_epoch_hour(t0);
        let back_ts = epoch_hour_to_timestamp(hour);
        assert_eq!(back_ts, t0);
    }

    #[test]
    fn test_build_synchronized_matrix_basic() {
        let base_us = 1724601600000000_i64;
        let h1 = base_us;
        let h2 = base_us + MICROS_PER_HOUR;
        let h3 = base_us + 2 * MICROS_PER_HOUR;

        let events = vec![
            // Ticker AAPL
            SentimentEvent {
                ticker: "AAPL".to_string(),
                sentiment_score: 0.8,
                timestamp_us: h1,
            },
            SentimentEvent {
                ticker: "AAPL".to_string(),
                sentiment_score: 0.6,
                timestamp_us: h1 + 100,
            },
            SentimentEvent {
                ticker: "AAPL".to_string(),
                sentiment_score: -0.4,
                timestamp_us: h2,
            },
            SentimentEvent {
                ticker: "AAPL".to_string(),
                sentiment_score: 0.5,
                timestamp_us: h3,
            },
            // Ticker MSFT
            SentimentEvent {
                ticker: "MSFT".to_string(),
                sentiment_score: 0.2,
                timestamp_us: h1,
            },
            SentimentEvent {
                ticker: "MSFT".to_string(),
                sentiment_score: 0.9,
                timestamp_us: h2,
            },
            SentimentEvent {
                ticker: "MSFT".to_string(),
                sentiment_score: -0.1,
                timestamp_us: h3,
            },
        ];

        let (timeline, matrix) = build_synchronized_matrix(&events, 2, 10);
        assert_eq!(timeline.len(), 3);
        assert!(matrix.contains_key("AAPL"));
        assert!(matrix.contains_key("MSFT"));

        let aapl = &matrix["AAPL"];
        assert_eq!(aapl.len(), 3);
        assert!((aapl[0] - 0.7).abs() < 1e-6); // (0.8 + 0.6) / 2
        assert!((aapl[1] - (-0.4)).abs() < 1e-6);
        assert!((aapl[2] - 0.5).abs() < 1e-6);

        let msft = &matrix["MSFT"];
        assert_eq!(msft.len(), 3);
        assert!((msft[0] - 0.2).abs() < 1e-6);
        assert!((msft[1] - 0.9).abs() < 1e-6);
        assert!((msft[2] - (-0.1)).abs() < 1e-6);
    }

    #[test]
    fn test_build_synchronized_matrix_filters_low_activity() {
        let base_us = 1724601600000000_i64;
        let events = vec![SentimentEvent {
            ticker: "RARE".to_string(),
            sentiment_score: 0.5,
            timestamp_us: base_us,
        }];

        // min_active_hours = 2 -> RARE should be excluded
        let (timeline, matrix) = build_synchronized_matrix(&events, 2, 10);
        assert!(timeline.is_empty());
        assert!(matrix.is_empty());
    }
}
