//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native QuestDB Influx Line Protocol (ILP) Sink
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::nlp::SentimentOutput;
use crate::pipeline::ProcessedDocument;
use chrono::{DateTime, Utc};
use reqwest::Client;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct QuestDbConfig {
    pub url: String,
    pub table_name: String,
    pub max_retries: usize,
    pub timeout_ms: u64,
}

impl Default for QuestDbConfig {
    fn default() -> Self {
        let url = env::var("QUESTDB_URL").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());
        let table_name = env::var("QUESTDB_TABLE").unwrap_or_else(|_| "sentiment_news".to_string());
        Self {
            url,
            table_name,
            max_retries: 3,
            timeout_ms: 3000,
        }
    }
}

pub struct QuestDbSink {
    client: Client,
    config: QuestDbConfig,
}

impl QuestDbSink {
    pub fn new(config: QuestDbConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        info!(
            "QuestDbSink initialized for endpoint: {} (table: '{}')",
            config.url, config.table_name
        );

        Self { client, config }
    }

    pub fn config(&self) -> &QuestDbConfig {
        &self.config
    }

    /// Escape tag keys and tag values according to Influx Line Protocol syntax.
    pub fn escape_tag_value(v: &str) -> String {
        v.replace(',', "\\,")
            .replace('=', "\\=")
            .replace(' ', "\\ ")
            .replace('\n', "")
            .replace('\r', "")
    }

    /// Escape string field values (enclosed in double quotes).
    pub fn escape_string_field(v: &str) -> String {
        v.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "")
    }

    /// Build a standard QuestDB Influx Line Protocol (ILP) formatted line.
    pub fn format_ilp_line(
        &self,
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> String {
        let ticker = doc
            .primary_ticker
            .as_deref()
            .unwrap_or_else(|| doc.tickers.first().map(|s| s.as_str()).unwrap_or("MARKET"));

        let tag_ticker = Self::escape_tag_value(ticker);
        let tag_source = Self::escape_tag_value(&doc.source);
        let tag_category = Self::escape_tag_value(&doc.event_category);

        let field_title = Self::escape_string_field(&doc.title);
        let field_clean_text = Self::escape_string_field(&doc.clean_text);
        let field_label = Self::escape_string_field(&sentiment.sentiment_label);

        let entities_json =
            serde_json::to_string(&doc.entities).unwrap_or_else(|_| "[]".to_string());
        let field_entities = Self::escape_string_field(&entities_json);

        let (pitch_mean, pitch_std, energy_mean, energy_std, pause_ratio, speech_rate) =
            if let Some(ref af) = doc.audio_features {
                (
                    af.pitch_mean,
                    af.pitch_std,
                    af.energy_mean,
                    af.energy_std,
                    af.pause_ratio,
                    af.speech_rate,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };

        let vpin = doc.vpin.unwrap_or(0.0);
        let gex = doc.gex.unwrap_or(0.0);
        let gex_positive = doc.gex_positive.unwrap_or(0.0);
        let gex_negative = doc.gex_negative.unwrap_or(0.0);

        // ILP designated timestamp: convert microseconds to nanoseconds
        let ts_nanos = signal_avail_ts_us * 1_000;

        let ingested_nanos = DateTime::parse_from_rfc3339(&doc.ingested_utc)
            .map(|dt| dt.timestamp_nanos_opt().unwrap_or(ts_nanos))
            .unwrap_or(ts_nanos);

        let db_commit_nanos = doc
            .db_commit_utc
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| {
                dt.timestamp_nanos_opt()
                    .unwrap_or_else(|| Utc::now().timestamp_nanos_opt().unwrap_or(ts_nanos))
            })
            .unwrap_or_else(|| Utc::now().timestamp_nanos_opt().unwrap_or(ts_nanos));

        let valid_from_nanos = doc
            .valid_from
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.timestamp_nanos_opt().unwrap_or(db_commit_nanos))
            .unwrap_or(db_commit_nanos);
        let valid_from_micros = valid_from_nanos / 1_000;

        let valid_to_part = if let Some(ref vt) = doc.valid_to {
            if let Ok(dt) = DateTime::parse_from_rfc3339(vt) {
                let vt_micros = dt.timestamp_nanos_opt().unwrap_or(0) / 1_000;
                format!(",valid_to={}t", vt_micros)
            } else {
                "".to_string()
            }
        } else {
            "".to_string()
        };

        let revision_number = doc.revision_number.unwrap_or(1);
        let is_current = doc.is_current.unwrap_or(true);

        format!(
            "{},ticker={},source={},event_category={} sentiment_score={:.4},sentiment_label=\"{}\",prob_positive={:.4},prob_negative={:.4},prob_neutral={:.4},title=\"{}\",summary_clean=\"{}\",entities=\"{}\",pitch_mean={:.4},pitch_std={:.4},energy_mean={:.4},energy_std={:.4},pause_ratio={:.4},speech_rate={:.4},vpin={:.4},gex={:.4},gex_positive={:.4},gex_negative={:.4},ingested_utc={}i,db_commit_utc={}i,valid_from={}t{},revision_number={}i,is_current={} {}",
            self.config.table_name,
            tag_ticker,
            tag_source,
            tag_category,
            sentiment.sentiment_score,
            field_label,
            sentiment.prob_positive,
            sentiment.prob_negative,
            sentiment.prob_neutral,
            field_title,
            field_clean_text,
            field_entities,
            pitch_mean,
            pitch_std,
            energy_mean,
            energy_std,
            pause_ratio,
            speech_rate,
            vpin,
            gex,
            gex_positive,
            gex_negative,
            ingested_nanos,
            db_commit_nanos,
            valid_from_micros,
            valid_to_part,
            revision_number,
            if is_current { "true" } else { "false" },
            ts_nanos
        )
    }

    /// Format a DailyBar record as Influx Line Protocol (ILP).
    pub fn format_stock_bar_ilp_line(&self, bar: &crate::sources::polygon::DailyBar) -> String {
        let tag_ticker = Self::escape_tag_value(&bar.ticker);
        let ts_nanos = bar.timestamp_ms * 1_000_000;
        format!(
            "stock_daily_bars,ticker={} open={:.4},high={:.4},low={:.4},close={:.4},volume={:.2},vwap={:.4} {}",
            tag_ticker,
            bar.open,
            bar.high,
            bar.low,
            bar.close,
            bar.volume,
            bar.vwap,
            ts_nanos
        )
    }

    /// Send a processed sentiment event to QuestDB via HTTP ILP endpoint with exponential backoff.
    pub async fn write_event(
        &self,
        doc: &ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Result<(), String> {
        let ilp_line = self.format_ilp_line(doc, sentiment, signal_avail_ts_us);
        self.send_ilp_payload(&ilp_line).await
    }

    /// Send a single daily stock bar to QuestDB via HTTP ILP.
    pub async fn write_stock_bar(
        &self,
        bar: &crate::sources::polygon::DailyBar,
    ) -> Result<(), String> {
        let ilp_line = self.format_stock_bar_ilp_line(bar);
        self.send_ilp_payload(&ilp_line).await
    }

    /// Send a batch of daily stock bars to QuestDB via HTTP ILP in a single payload.
    pub async fn write_stock_bars_batch(
        &self,
        bars: &[crate::sources::polygon::DailyBar],
    ) -> Result<(), String> {
        if bars.is_empty() {
            return Ok(());
        }
        let lines: Vec<String> = bars
            .iter()
            .map(|b| self.format_stock_bar_ilp_line(b))
            .collect();
        let payload = lines.join("\n") + "\n";
        self.send_ilp_payload(&payload).await
    }

    /// Format a SignalLatencyMetrics record as Influx Line Protocol (ILP).
    pub fn format_latency_metric_ilp_line(
        &self,
        doc: &ProcessedDocument,
        latency: &crate::pipeline::SignalLatencyMetrics,
        ts_nanos: i64,
    ) -> String {
        let ticker = doc
            .primary_ticker
            .as_deref()
            .unwrap_or_else(|| doc.tickers.first().map(|s| s.as_str()).unwrap_or("MARKET"));

        let tag_ticker = Self::escape_tag_value(ticker);
        let tag_source = Self::escape_tag_value(&doc.source);

        format!(
            "signal_latency_metrics,ticker={},source={} fetch_latency_ms={:.4},normalization_latency_ms={:.4},inference_latency_ms={:.4},write_latency_ms={:.4},total_signal_latency_ms={:.4},sla_target_ms={}i,is_sla_compliant={} {}",
            tag_ticker,
            tag_source,
            latency.fetch_latency_ms,
            latency.normalization_latency_ms,
            latency.inference_latency_ms,
            latency.write_latency_ms,
            latency.total_signal_latency_ms,
            latency.sla_target_ms,
            if latency.is_sla_compliant { "true" } else { "false" },
            ts_nanos
        )
    }

    /// Send a single latency metric record to QuestDB via HTTP ILP.
    pub async fn write_latency_metric(
        &self,
        doc: &ProcessedDocument,
        latency: &crate::pipeline::SignalLatencyMetrics,
        ts_nanos: i64,
    ) -> Result<(), String> {
        let ilp_line = self.format_latency_metric_ilp_line(doc, latency, ts_nanos);
        self.send_ilp_payload(&ilp_line).await
    }

    /// Helper to send an ILP text payload to QuestDB with retry and exponential backoff.
    pub async fn send_ilp_payload(&self, payload: &str) -> Result<(), String> {
        let endpoint = format!(
            "{}/write?precision=n",
            self.config.url.trim_end_matches('/')
        );
        let mut backoff = Duration::from_millis(100);

        for attempt in 1..=self.config.max_retries {
            let resp_res = self
                .client
                .post(&endpoint)
                .body(payload.to_string())
                .header("Content-Type", "text/plain; charset=utf-8")
                .send()
                .await;

            match resp_res {
                Ok(resp) => {
                    if resp.status().is_success() {
                        return Ok(());
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        warn!(
                            "QuestDB HTTP write attempt {} failed with status {}: {}",
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
            "Failed to write ILP payload to QuestDB at {} after {} attempts",
            self.config.url, self.config.max_retries
        ))
    }

    /// Ingest a sentiment revision according to Slowly Changing Dimension Type 2 (SCD2):
    /// 1. Look up any existing active record (`is_current = true`) for the natural key.
    /// 2. If an active record is found:
    ///    - Supersede it via SQL UPDATE by setting `valid_to = new_valid_from` and `is_current = false`.
    ///    - Prepare new version with `revision_number = previous_revision + 1`, `valid_from = new_valid_from`, `valid_to = None`, `is_current = true`.
    /// 3. If no active record is found:
    ///    - Prepare initial version with `revision_number = 1`, `valid_from = db_commit_utc`, `valid_to = None`, `is_current = true`.
    /// 4. Write the new version ILP line to QuestDB.
    pub async fn upsert_sentiment_revision(
        &self,
        doc: &mut ProcessedDocument,
        sentiment: &SentimentOutput,
        signal_avail_ts_us: i64,
    ) -> Result<(i32, String), String> {
        let now_utc = Utc::now().to_rfc3339();
        if doc.db_commit_utc.is_none() {
            doc.db_commit_utc = Some(now_utc.clone());
        }
        let valid_from_str = doc.valid_from.clone().unwrap_or_else(|| now_utc.clone());
        doc.valid_from = Some(valid_from_str.clone());

        let ticker = doc
            .primary_ticker
            .as_deref()
            .unwrap_or_else(|| doc.tickers.first().map(|s| s.as_str()).unwrap_or("MARKET"));

        // Query existing active record revision
        let exec_url = format!("{}/exec", self.config.url.trim_end_matches('/'));
        let check_sql = format!(
            "SELECT revision_number, valid_from FROM {} WHERE ticker = '{}' AND source = '{}' AND is_current = true ORDER BY timestamp DESC LIMIT 1;",
            self.config.table_name,
            Self::escape_tag_value(ticker),
            Self::escape_tag_value(&doc.source),
        );

        let mut next_revision = 1;
        if let Ok(resp) = self
            .client
            .get(&exec_url)
            .query(&[("query", &check_sql)])
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(dataset) = json.get("dataset").and_then(|d| d.as_array()) {
                        if let Some(first_row) = dataset.first().and_then(|r| r.as_array()) {
                            if let Some(prev_rev) = first_row.first().and_then(|v| v.as_i64()) {
                                next_revision = (prev_rev as i32) + 1;

                                // Supersede old record
                                let update_sql = format!(
                                    "UPDATE {} SET valid_to = '{}', is_current = false WHERE ticker = '{}' AND source = '{}' AND is_current = true;",
                                    self.config.table_name,
                                    valid_from_str,
                                    Self::escape_tag_value(ticker),
                                    Self::escape_tag_value(&doc.source),
                                );
                                let _ = self
                                    .client
                                    .get(&exec_url)
                                    .query(&[("query", &update_sql)])
                                    .send()
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        doc.revision_number = Some(next_revision);
        doc.is_current = Some(true);
        doc.valid_to = None;

        let ilp_line = self.format_ilp_line(doc, sentiment, signal_avail_ts_us);
        self.send_ilp_payload(&ilp_line).await?;

        Ok((next_revision, valid_from_str))
    }

    /// Ensures required tables exist in QuestDB (`sentiment_news`, `stock_daily_bars`, `signal_latency_metrics`).
    pub async fn initialize_tables(&self) -> Result<(), String> {
        let create_sentiment_news_sql = "CREATE TABLE IF NOT EXISTS sentiment_news (ticker SYMBOL, source SYMBOL, event_category SYMBOL, sentiment_score DOUBLE, sentiment_label STRING, prob_positive DOUBLE, prob_negative DOUBLE, prob_neutral DOUBLE, title STRING, summary_clean STRING, entities STRING, pitch_mean DOUBLE, pitch_std DOUBLE, energy_mean DOUBLE, energy_std DOUBLE, pause_ratio DOUBLE, speech_rate DOUBLE, vpin DOUBLE, gex DOUBLE, gex_positive DOUBLE, gex_negative DOUBLE, ingested_utc LONG, db_commit_utc LONG, valid_from TIMESTAMP, valid_to TIMESTAMP, revision_number INT, is_current BOOLEAN, timestamp TIMESTAMP) TIMESTAMP(timestamp) PARTITION BY DAY;";
        let create_stock_bars_sql = "CREATE TABLE IF NOT EXISTS stock_daily_bars (ticker SYMBOL, open DOUBLE, high DOUBLE, low DOUBLE, close DOUBLE, volume DOUBLE, vwap DOUBLE, date TIMESTAMP) TIMESTAMP(date) PARTITION BY YEAR;";
        let create_latency_sql = "CREATE TABLE IF NOT EXISTS signal_latency_metrics (ticker SYMBOL, source SYMBOL, fetch_latency_ms DOUBLE, normalization_latency_ms DOUBLE, inference_latency_ms DOUBLE, write_latency_ms DOUBLE, total_signal_latency_ms DOUBLE, sla_target_ms INT, is_sla_compliant BOOLEAN, timestamp TIMESTAMP) TIMESTAMP(timestamp) PARTITION BY DAY;";

        let exec_url = format!("{}/exec", self.config.url.trim_end_matches('/'));

        for sql in [
            create_sentiment_news_sql,
            create_stock_bars_sql,
            create_latency_sql,
        ] {
            match self
                .client
                .get(&exec_url)
                .query(&[("query", sql)])
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        info!("QuestDB table verified/initialized");
                    } else {
                        let err = resp.text().await.unwrap_or_default();
                        warn!("QuestDB table initialization status {}: {}", status, err);
                    }
                }
                Err(e) => {
                    warn!(
                        "Could not initialize QuestDB tables directly (offline/mock mode): {}",
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_ilp_line_formatting() {
        let config = QuestDbConfig {
            url: "http://127.0.0.1:9000".to_string(),
            table_name: "sentiment_news".to_string(),
            max_retries: 3,
            timeout_ms: 1000,
        };
        let sink = QuestDbSink::new(config);

        let doc = ProcessedDocument {
            id: "doc-123".to_string(),
            title: "NVIDIA Beats Revenue Estimates as $NVDA AI Chip Demand Surges".to_string(),
            source: "MarketWatch".to_string(),
            url: "https://marketwatch.com/nvda".to_string(),
            published_utc: "2026-08-25T14:30:00Z".to_string(),
            ingested_utc: "2026-08-25T14:30:00.050Z".to_string(),
            db_commit_utc: Some("2026-08-25T14:30:00.100Z".to_string()),
            clean_text: "NVIDIA posted record quarterly revenue.".to_string(),
            extracted_links: vec![],
            tickers: vec!["NVDA".to_string()],
            primary_ticker: Some("NVDA".to_string()),
            ticker_confidences: vec![("NVDA".to_string(), 0.99)],
            is_spam: false,
            spam_reason: "Clean".to_string(),
            event_category: "EARNINGS".to_string(),
            preprocessing_latency_us: 350,
            entities: vec![],
            audio_transcript: None,
            audio_features: None,
            vpin: Some(0.6543),
            gex: Some(1250000.0),
            gex_positive: Some(1500000.0),
            gex_negative: Some(250000.0),
            latency_metrics: None,
            ..Default::default()
        };

        let sentiment = SentimentOutput {
            sentiment_score: 0.85,
            sentiment_label: "POSITIVE".to_string(),
            prob_positive: 0.90,
            prob_negative: 0.05,
            prob_neutral: 0.05,
            ..Default::default()
        };

        let ilp = sink.format_ilp_line(&doc, &sentiment, 1724601600000000);
        assert!(ilp
            .starts_with("sentiment_news,ticker=NVDA,source=MarketWatch,event_category=EARNINGS"));
        assert!(ilp.contains("sentiment_score=0.8500"));
        assert!(ilp.contains("sentiment_label=\"POSITIVE\""));
        assert!(
            ilp.contains("title=\"NVIDIA Beats Revenue Estimates as $NVDA AI Chip Demand Surges\"")
        );
        assert!(ilp.contains("entities=\"[]\""));
        assert!(ilp.contains("pitch_mean=0.0000"));
        assert!(ilp.contains("energy_mean=0.0000"));
        assert!(ilp.contains("pause_ratio=0.0000"));
        assert!(ilp.contains("speech_rate=0.0000"));
        assert!(ilp.contains("vpin=0.6543"));
        assert!(ilp.contains("gex=1250000.0000"));
        assert!(ilp.contains("gex_positive=1500000.0000"));
        assert!(ilp.contains("gex_negative=250000.0000"));
        assert!(ilp.contains("ingested_utc=1787668200050000000i"));
        assert!(ilp.contains("db_commit_utc=1787668200100000000i"));
        assert!(ilp.contains("valid_from=1787668200100000t"));
        assert!(ilp.contains("revision_number=1i"));
        assert!(ilp.contains("is_current=true"));
        assert!(!ilp.contains("valid_to="));
        assert!(ilp.ends_with(" 1724601600000000000"));
    }

    #[test]
    fn test_scd2_superseded_revision_ilp_formatting() {
        let config = QuestDbConfig {
            url: "http://127.0.0.1:9000".to_string(),
            table_name: "sentiment_news".to_string(),
            max_retries: 3,
            timeout_ms: 1000,
        };
        let sink = QuestDbSink::new(config);

        let mut doc = ProcessedDocument {
            id: "doc-rev-2".to_string(),
            title: "Apple Reports Revised Higher Services Revenue".to_string(),
            source: "SEC EDGAR".to_string(),
            url: "https://sec.gov/edgar/aapl".to_string(),
            published_utc: "2026-08-25T14:30:00Z".to_string(),
            ingested_utc: "2026-08-25T14:30:00.050Z".to_string(),
            db_commit_utc: Some("2026-08-25T14:30:00.100Z".to_string()),
            clean_text: "Apple services climbed 16% in revised filing".to_string(),
            extracted_links: vec![],
            tickers: vec!["AAPL".to_string()],
            primary_ticker: Some("AAPL".to_string()),
            ticker_confidences: vec![("AAPL".to_string(), 0.99)],
            is_spam: false,
            spam_reason: "Clean".to_string(),
            event_category: "EARNINGS".to_string(),
            preprocessing_latency_us: 120,
            entities: vec![],
            audio_transcript: None,
            audio_features: None,
            vpin: None,
            gex: None,
            gex_positive: None,
            gex_negative: None,
            latency_metrics: None,
            source_id: Some("sec-aapl-8ka-001".to_string()),
            valid_from: Some("2026-08-25T14:30:00.100Z".to_string()),
            valid_to: Some("2026-08-26T09:15:00.000Z".to_string()),
            revision_number: Some(1),
            is_current: Some(false),
            ..Default::default()
        };

        let sentiment = SentimentOutput {
            sentiment_score: 0.65,
            sentiment_label: "POSITIVE".to_string(),
            prob_positive: 0.85,
            prob_negative: 0.05,
            prob_neutral: 0.10,
            ..Default::default()
        };

        let ilp = sink.format_ilp_line(&doc, &sentiment, 1724601600000000);
        assert!(ilp.contains("valid_from=1787668200100000t"));
        assert!(ilp.contains("valid_to=1787735700000000t"));
        assert!(ilp.contains("revision_number=1i"));
        assert!(ilp.contains("is_current=false"));

        // Now simulate new version 2 superseding version 1
        doc.valid_from = Some("2026-08-26T09:15:00.000Z".to_string());
        doc.valid_to = None;
        doc.revision_number = Some(2);
        doc.is_current = Some(true);

        let ilp_v2 = sink.format_ilp_line(&doc, &sentiment, 1724601600000000);
        assert!(ilp_v2.contains("valid_from=1787735700000000t"));
        assert!(!ilp_v2.contains("valid_to="));
        assert!(ilp_v2.contains("revision_number=2i"));
        assert!(ilp_v2.contains("is_current=true"));
    }

    #[tokio::test]
    async fn test_questdb_offline_graceful_error() {
        let config = QuestDbConfig {
            url: "http://127.0.0.1:9999".to_string(), // non-existent port
            table_name: "sentiment_news".to_string(),
            max_retries: 1,
            timeout_ms: 100,
        };
        let sink = QuestDbSink::new(config);

        let doc = ProcessedDocument {
            id: "doc-123".to_string(),
            title: "Test headline".to_string(),
            source: "Test".to_string(),
            url: "http://test.com".to_string(),
            published_utc: "2026-08-25T14:30:00Z".to_string(),
            ingested_utc: "2026-08-25T14:30:00.050Z".to_string(),
            db_commit_utc: None,
            clean_text: "Clean text".to_string(),
            extracted_links: vec![],
            tickers: vec!["AAPL".to_string()],
            primary_ticker: Some("AAPL".to_string()),
            ticker_confidences: vec![],
            is_spam: false,
            spam_reason: "Clean".to_string(),
            event_category: "GENERAL".to_string(),
            preprocessing_latency_us: 100,
            entities: vec![],
            audio_transcript: None,
            audio_features: None,
            vpin: None,
            gex: None,
            gex_positive: None,
            gex_negative: None,
            latency_metrics: None,
            ..Default::default()
        };

        let sentiment = SentimentOutput {
            sentiment_score: 0.5,
            sentiment_label: "POSITIVE".to_string(),
            prob_positive: 0.7,
            prob_negative: 0.2,
            prob_neutral: 0.1,
            ..Default::default()
        };

        let res = sink.write_event(&doc, &sentiment, 1724601600000000).await;
        assert!(res.is_err());
        let err_msg = res.unwrap_err();
        assert!(
            err_msg.contains("Failed to write ILP payload to QuestDB")
                || err_msg.contains("Failed to write event to QuestDB")
        );
    }

    #[test]
    fn test_stock_bar_ilp_formatting() {
        let config = QuestDbConfig {
            url: "http://127.0.0.1:9000".to_string(),
            table_name: "sentiment_news".to_string(),
            max_retries: 3,
            timeout_ms: 1000,
        };
        let sink = QuestDbSink::new(config);

        let bar = crate::sources::polygon::DailyBar {
            ticker: "AAPL".to_string(),
            date: "2025-01-02".to_string(),
            timestamp_ms: 1735833600000,
            open: 224.50,
            high: 226.80,
            low: 223.10,
            close: 225.40,
            volume: 48500000.0,
            vwap: 225.10,
        };

        let ilp = sink.format_stock_bar_ilp_line(&bar);
        assert!(ilp.starts_with("stock_daily_bars,ticker=AAPL"));
        assert!(ilp.contains("open=224.5000"));
        assert!(ilp.contains("high=226.8000"));
        assert!(ilp.contains("low=223.1000"));
        assert!(ilp.contains("close=225.4000"));
        assert!(ilp.contains("volume=48500000.00"));
        assert!(ilp.contains("vwap=225.1000"));
        assert!(ilp.ends_with("1735833600000000000"));
    }

    #[test]
    fn test_latency_metrics_ilp_formatting() {
        let config = QuestDbConfig {
            url: "http://127.0.0.1:9000".to_string(),
            table_name: "sentiment_news".to_string(),
            max_retries: 3,
            timeout_ms: 1000,
        };
        let sink = QuestDbSink::new(config);

        let doc = ProcessedDocument {
            id: "doc-latency-1".to_string(),
            title: "Apple Reports Strong Services Revenue Growth".to_string(),
            source: "Finnhub".to_string(),
            url: "https://finnhub.io/news/1".to_string(),
            published_utc: "2026-08-25T14:30:00Z".to_string(),
            ingested_utc: "2026-08-25T14:30:00.020Z".to_string(),
            db_commit_utc: Some("2026-08-25T14:30:00.075Z".to_string()),
            clean_text: "Apple services climbed 14%".to_string(),
            extracted_links: vec![],
            tickers: vec!["AAPL".to_string()],
            primary_ticker: Some("AAPL".to_string()),
            ticker_confidences: vec![("AAPL".to_string(), 0.98)],
            is_spam: false,
            spam_reason: "Clean".to_string(),
            event_category: "EARNINGS".to_string(),
            preprocessing_latency_us: 200,
            entities: vec![],
            audio_transcript: None,
            audio_features: None,
            vpin: None,
            gex: None,
            gex_positive: None,
            gex_negative: None,
            latency_metrics: None,
            ..Default::default()
        };

        let metrics = crate::pipeline::SignalLatencyMetrics {
            fetch_latency_ms: 18.5,
            normalization_latency_ms: 3.2,
            inference_latency_ms: 12.1,
            write_latency_ms: 4.8,
            total_signal_latency_ms: 75.0,
            sla_target_ms: 500,
            is_sla_compliant: true,
        };

        let ilp = sink.format_latency_metric_ilp_line(&doc, &metrics, 1724601600000000000);
        assert!(ilp.starts_with("signal_latency_metrics,ticker=AAPL,source=Finnhub"));
        assert!(ilp.contains("fetch_latency_ms=18.5000"));
        assert!(ilp.contains("normalization_latency_ms=3.2000"));
        assert!(ilp.contains("inference_latency_ms=12.1000"));
        assert!(ilp.contains("write_latency_ms=4.8000"));
        assert!(ilp.contains("total_signal_latency_ms=75.0000"));
        assert!(ilp.contains("sla_target_ms=500i"));
        assert!(ilp.contains("is_sla_compliant=true"));
        assert!(ilp.ends_with(" 1724601600000000000"));
    }
}
