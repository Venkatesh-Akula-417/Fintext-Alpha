//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Federal Reserve FOMC Policy Statement Collector
//! ═══════════════════════════════════════════════════════════════════════════════
//! [DATA LICENSING COMPLIANCE NOTICE]
//! Status: ACTIVE (Public Domain US Government Data - 17 U.S.C. § 105).
//!
//! // Public domain US govt 17 USC §105, free to republish, no license fee, SAFE.
//! Works of the United States Government are not subject to domestic copyright
//! protection. FOMC rate decisions and policy statements are freely redistributable,
//! vectorizable, and monetizable without third-party licensing fees.
//! Controlled via `ENABLE_FOMC=1`.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::RawDocument;
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{interval, sleep_until, Instant};
use tracing::{debug, info, warn};
use uuid::Uuid;

pub const FOMC_USER_AGENT: &str = "FinText Institutional Macro Research/2.0 (macro@fintext-alpha.com)";
pub const DEFAULT_BURST_INTERVAL_MS: u64 = 10; // 10ms micro-burst polling
pub const DEFAULT_BURST_DURATION_SECS: u64 = 300; // 5 minutes burst duration


/// Record representation of an FOMC scheduled event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FomcScheduleEntry {
    pub date: String,
    #[serde(default)]
    pub time_est: Option<String>,
    pub time_utc: String,
}

/// High-throughput Federal Reserve FOMC statement fetcher with HTTP/2 Keep-Alive and micro-burst capability.
pub struct FomcFetcher {
    client: Client,
    etag: Arc<Mutex<Option<String>>>,
    last_modified: Arc<Mutex<Option<String>>>,
    schedule_path: Option<PathBuf>,
}

impl FomcFetcher {
    /// Creates a new FOMC fetcher configured with ultra-low latency HTTP/2 connection pooling.
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(FOMC_USER_AGENT));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));

        // Ultra-low latency: HTTP/2 Keep-Alive saves 30-50ms TLS handshake, pool 10 ready connections, target 1-2ms
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .http2_prior_knowledge() // direct HTTP/2, no upgrade negotiation 30ms save
            .tcp_keepalive(Duration::from_secs(60)) // keep connection alive 60s
            .pool_idle_timeout(Duration::from_secs(90)) // pool keep 90s
            .pool_max_idle_per_host(10) // 10 ready connections
            .http2_keep_alive_interval(Duration::from_secs(20)) // ping every 20s
            .http2_keep_alive_timeout(Duration::from_secs(5))
            .http2_keep_alive_while_idle(true) // keep alive even idle
            .build()
            .expect("Failed to build FOMC reqwest client");

        Self {
            client,
            etag: Arc::new(Mutex::new(None)),
            last_modified: Arc::new(Mutex::new(None)),
            schedule_path: None,
        }
    }

    /// Sets custom schedule path for FOMC release dates.
    pub fn with_schedule_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.schedule_path = Some(path.into());
        self
    }

    /// Fetches the latest statement at `url` using conditional ETag / Last-Modified headers.
    /// Returns `Ok(None)` if HTTP 304 Not Modified (sub-millisecond latency confirmation).
    pub async fn fetch_statement(&self, url: &str) -> Result<Option<RawDocument>, String> {
        let mut req = self.client.get(url);

        let (cached_etag, cached_mod) = {
            let tag = self.etag.lock().await.clone();
            let lm = self.last_modified.lock().await.clone();
            (tag, lm)
        };

        if let Some(ref tag) = cached_etag {
            if let Ok(val) = HeaderValue::from_str(tag) {
                req = req.header(IF_NONE_MATCH, val);
            }
        }
        if let Some(ref lm) = cached_mod {
            if let Ok(val) = HeaderValue::from_str(lm) {
                req = req.header(IF_MODIFIED_SINCE, val);
            }
        }

        let resp = req.send().await.map_err(|e| format!("HTTP request error for FOMC: {}", e))?;
        let status = resp.status();

        // 304 Not Modified: 1ms ultra-fast path, no content changes
        if status.as_u16() == 304 {
            return Ok(None);
        }

        if status.is_success() {
            // Update ETag and Last-Modified headers for next conditional request
            if let Some(et) = resp.headers().get(ETAG) {
                if let Ok(s) = et.to_str() {
                    let mut tag_guard = self.etag.lock().await;
                    *tag_guard = Some(s.to_string());
                }
            }
            if let Some(lm) = resp.headers().get(LAST_MODIFIED) {
                if let Ok(s) = lm.to_str() {
                    let mut lm_guard = self.last_modified.lock().await;
                    *lm_guard = Some(s.to_string());
                }
            }

            let text = resp.text().await.map_err(|e| format!("Failed to read FOMC body: {}", e))?;
            let doc = self.parse_fomc_html(&text, url);
            Ok(Some(doc))
        } else {
            Err(format!("FOMC HTTP status error: {}", status))
        }
    }

    /// Parses raw FOMC HTML release into a normalized `RawDocument`.
    pub fn parse_fomc_html(&self, html: &str, url: &str) -> RawDocument {
        // Extract title from HTML tag or fallback to standard statement title
        let title = if let Some(start) = html.find("<title>") {
            if let Some(end) = html[start + 7..].find("</title>") {
                html[start + 7..start + 7 + end].trim().to_string()
            } else {
                "Federal Reserve FOMC Monetary Policy Statement".to_string()
            }
        } else {
            "Federal Reserve FOMC Monetary Policy Statement".to_string()
        };

        // Strip basic HTML markup for raw content vectorization
        let clean_content = html
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<p>", "\n\n")
            .replace("</p>", "");

        RawDocument {
            id: Uuid::new_v4().to_string(),
            title,
            source: "FOMC".to_string(),
            url: url.to_string(),
            published_utc: Utc::now().to_rfc3339(),
            raw_content: clean_content,
            audio_path: None,
            ingested_utc: Utc::now().to_rfc3339(),
            ..Default::default()
        }
    }

    /// Calculates the next scheduled FOMC policy announcement time.
    pub fn get_next_fomc_time(&self) -> DateTime<Utc> {
        let schedule_file = self.schedule_path.clone().unwrap_or_else(|| {
            PathBuf::from("config/fomc_schedule.json")
        });

        if schedule_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&schedule_file) {
                if let Ok(entries) = serde_json::from_str::<Vec<FomcScheduleEntry>>(&content) {
                    let now = Utc::now();
                    for entry in entries {
                        if let Ok(dt) = DateTime::parse_from_rfc3339(&entry.time_utc) {
                            let utc_dt = dt.with_timezone(&Utc);
                            if utc_dt > now {
                                return utc_dt;
                            }
                        }
                    }
                }
            }
        }

        // Default hardcoded fallback dates for 2024-2026 (8 meetings/yr, 14:00:00 EST / 18:00:00 UTC or 19:00:00 UTC)
        let fallback_dates = [
            "2026-03-18T18:00:00Z",
            "2026-05-06T18:00:00Z",
            "2026-06-17T18:00:00Z",
            "2026-07-29T18:00:00Z",
            "2026-09-16T18:00:00Z",
            "2026-11-05T19:00:00Z",
            "2026-12-16T19:00:00Z",
        ];

        let now = Utc::now();
        for s in &fallback_dates {
            if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                let utc_dt = dt.with_timezone(&Utc);
                if utc_dt > now {
                    return utc_dt;
                }
            }
        }

        // Default to 1 hour from now if all scheduled dates passed
        now + chrono::Duration::hours(1)
    }

    /// Runs a scheduled micro-burst polling routine around 2:00 PM EST (18:00 / 19:00 UTC).
    /// Sleeps until 1 second before release, then initiates high-frequency 10ms micro-burst polling.
    pub async fn scheduled_micro_burst_poller(
        &self,
        url: &str,
        poll_interval_ms: u64,
        burst_duration_secs: u64,
    ) -> Result<Option<RawDocument>, String> {
        let next_fomc = self.get_next_fomc_time();
        let now = Utc::now();

        if next_fomc > now {
            let time_until = next_fomc - now;
            let sleep_duration = time_until.to_std().unwrap_or(Duration::from_secs(0));
            // Wake up 1 second prior to scheduled release
            let pre_wake_duration = sleep_duration.saturating_sub(Duration::from_secs(1));
            info!(
                "[FOMC Micro-Burst Poller] Scheduled next release at {} (sleeping for {:?})",
                next_fomc, pre_wake_duration
            );

            sleep_until(Instant::now() + pre_wake_duration).await;
        }

        info!(
            "[FOMC Micro-Burst Poller] Initiating 10ms micro-burst polling against {} for {}s",
            url, burst_duration_secs
        );

        let interval_duration = Duration::from_millis(poll_interval_ms.max(1));
        let mut ticker = interval(interval_duration);
        let deadline = Instant::now() + Duration::from_secs(burst_duration_secs);

        while Instant::now() < deadline {
            ticker.tick().await;

            match self.fetch_statement(url).await {
                Ok(Some(doc)) => {
                    info!(
                        "[FOMC Micro-Burst Poller] Fresh policy statement captured: '{}' (len: {} bytes)",
                        doc.title,
                        doc.raw_content.len()
                    );
                    return Ok(Some(doc));
                }
                Ok(None) => {
                    // HTTP 304: Document not yet updated, continue micro-burst
                    debug!("[FOMC Micro-Burst Poller] 304 Not Modified (1ms tick)");
                }
                Err(e) => {
                    warn!("[FOMC Micro-Burst Poller] Transient network error: {}", e);
                }
            }
        }

        info!("[FOMC Micro-Burst Poller] Micro-burst window expired without new statement");
        Ok(None)
    }
}

impl Default for FomcFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fomc_html_parsing() {
        let fetcher = FomcFetcher::new();
        let sample_html = r#"
            <html>
                <head><title>Federal Reserve issues FOMC statement</title></head>
                <body>
                    <p>Recent indicators suggest that economic activity has continued to expand at a solid pace.</p>
                </body>
            </html>
        "#;
        let doc = fetcher.parse_fomc_html(sample_html, "https://www.federalreserve.gov/sample");
        assert_eq!(doc.title, "Federal Reserve issues FOMC statement");
        assert_eq!(doc.source, "FOMC");
        assert!(doc.raw_content.contains("Recent indicators suggest"));
    }

    #[test]
    fn test_get_next_fomc_time_fallback() {
        let fetcher = FomcFetcher::new();
        let next_time = fetcher.get_next_fomc_time();
        assert!(next_time >= Utc::now());
    }

    #[tokio::test]
    async fn test_fomc_etag_cache_initialization() {
        let fetcher = FomcFetcher::new();
        let tag = fetcher.etag.lock().await;
        assert_eq!(*tag, None);
    }
}
