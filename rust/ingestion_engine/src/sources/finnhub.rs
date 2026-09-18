//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Finnhub Real-Time Financial News Poller
//! ═══════════════════════════════════════════════════════════════════════════════
//! [DATA LICENSING COMPLIANCE NOTICE]
//! Status: ACTIVE (Internal Quantitative Analytics Use).
//!
//! Finnhub market news ingestion is active for internal quantitative signal extraction.
//! Note: A commercial distribution license with Finnhub is required prior to external
//! raw document or signal redistribution. Controlled via `ENABLE_FINNHUB=1`.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::RawDocument;
use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, warn};
use uuid::Uuid;

const FINNHUB_USER_AGENT: &str = "FinText-Alpha-Vectorizer/2.0 (Institutional Ingestion Engine)";
const DEFAULT_TIMEOUT_SECS: u64 = 10;
const MAX_RETRIES: usize = 2;

/// Intermediate deserialization model for Finnhub Market News API payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinnhubArticle {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub category: Option<String>,
    pub datetime: i64,
    pub headline: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub related: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub url: String,
}

impl From<FinnhubArticle> for RawDocument {
    fn from(article: FinnhubArticle) -> Self {
        let published_utc = DateTime::<Utc>::from_timestamp(article.datetime, 0)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        let doc_id = if let Some(id_num) = article.id {
            format!("finnhub-{}", id_num)
        } else {
            Uuid::new_v4().to_string()
        };

        let source_name = if article.source.trim().is_empty() {
            "Finnhub".to_string()
        } else {
            format!("Finnhub-{}", article.source.trim())
        };

        RawDocument {
            id: doc_id,
            title: article.headline.trim().to_string(),
            source: source_name,
            url: article.url.trim().to_string(),
            published_utc,
            raw_content: article.summary.trim().to_string(),
            audio_path: None,
            ingested_utc: Utc::now().to_rfc3339(),
            ..Default::default()
        }
    }
}

/// Finnhub REST API client for streaming real-time general and company news.
#[derive(Debug, Clone)]
pub struct FinnhubClient {
    api_key: String,
    client: Client,
}

impl FinnhubClient {
    /// Initialize Finnhub client with a specified API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(FINNHUB_USER_AGENT));

        // Ultra-low latency: HTTP/2 Keep-Alive saves 30-50ms TLS handshake, pool 10 ready connections, target 1-2ms
        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .http2_prior_knowledge() // direct HTTP/2, no upgrade negotiation 30ms save
            .tcp_keepalive(Duration::from_secs(60)) // keep connection alive 60s
            .pool_idle_timeout(Duration::from_secs(90)) // pool keep 90s
            .pool_max_idle_per_host(10) // 10 ready connections
            .http2_keep_alive_interval(Duration::from_secs(20)) // ping every 20s
            .http2_keep_alive_timeout(Duration::from_secs(5))
            .http2_keep_alive_while_idle(true) // keep alive even idle
            .build()
            .expect("Failed to build Finnhub reqwest client");

        Self {
            api_key: api_key.into(),
            client,
        }
    }

    /// Read `FINNHUB_API_KEY` from environment. Returns `Err` if missing or empty.
    pub fn from_env() -> Result<Self, String> {
        let key = env::var("FINNHUB_API_KEY")
            .map_err(|_| "FINNHUB_API_KEY environment variable is not set".to_string())?;

        let trimmed = key.trim().to_string();
        if trimmed.is_empty() || trimmed == "your_finnhub_api_key_here" {
            return Err("FINNHUB_API_KEY is empty or placeholder".to_string());
        }

        Ok(Self::new(trimmed))
    }

    /// Access the API key string (for debugging/masking).
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Fetch latest market news for a given category (default: "general") with exponential backoff and rate-limiting resilience.
    pub async fn fetch_news(
        &self,
        category: &str,
        limit: usize,
    ) -> Result<Vec<RawDocument>, String> {
        let clean_category = if category.trim().is_empty() {
            "general"
        } else {
            category.trim()
        };

        let url = format!(
            "https://finnhub.io/api/v1/news?category={}&token={}",
            clean_category, self.api_key
        );

        let mut retries = 0;
        let mut delay = Duration::from_millis(500);

        loop {
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let text = resp
                            .text()
                            .await
                            .map_err(|e| format!("Failed to read Finnhub response body: {}", e))?;
                        return parse_finnhub_news_json(&text, limit);
                    } else if status.as_u16() == 429 {
                        warn!(
                            "[Finnhub] Rate limit hit (HTTP 429). Backing off for 30s before retry (attempt {}/{})",
                            retries + 1,
                            MAX_RETRIES
                        );
                        retries += 1;
                        if retries > MAX_RETRIES {
                            return Err(format!(
                                "Finnhub rate limit (HTTP 429) persisted after {} retries",
                                MAX_RETRIES
                            ));
                        }
                        sleep(Duration::from_secs(30)).await;
                    } else if status.is_server_error() {
                        warn!(
                            "[Finnhub] Server error (HTTP {}). Retrying in {:?} (attempt {}/{})",
                            status,
                            delay,
                            retries + 1,
                            MAX_RETRIES
                        );
                        retries += 1;
                        if retries > MAX_RETRIES {
                            return Err(format!(
                                "Finnhub server error HTTP {} exceeded max retries",
                                status
                            ));
                        }
                        sleep(delay).await;
                        delay *= 2;
                    } else {
                        error!("[Finnhub] Permanent HTTP error: status={}", status);
                        return Err(format!(
                            "Finnhub API returned permanent error HTTP {}",
                            status
                        ));
                    }
                }
                Err(err) => {
                    retries += 1;
                    if retries > MAX_RETRIES {
                        return Err(format!(
                            "Network error connecting to Finnhub after {} retries: {}",
                            MAX_RETRIES, err
                        ));
                    }
                    warn!(
                        "[Finnhub] Network connection error: {}. Retrying in {:?}...",
                        err, delay
                    );
                    sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }
}

/// Helper to parse Finnhub JSON news array and convert to `RawDocument` vector.
pub fn parse_finnhub_news_json(json_str: &str, limit: usize) -> Result<Vec<RawDocument>, String> {
    let articles: Vec<FinnhubArticle> = serde_json::from_str(json_str)
        .map_err(|e| format!("Finnhub JSON deserialization error: {}", e))?;

    let count = if limit == 0 {
        articles.len()
    } else {
        limit.min(articles.len())
    };

    let docs: Vec<RawDocument> = articles
        .into_iter()
        .take(count)
        .filter(|a| !a.headline.trim().is_empty())
        .map(RawDocument::from)
        .collect();

    Ok(docs)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    const SAMPLE_FINNHUB_JSON: &str = r#"[
        {
            "category": "company",
            "datetime": 1724658000,
            "headline": "Apple Unveils Next-Generation M4 Chip with Advanced Neural Engine for Mac",
            "id": 7891234,
            "image": "https://img.finnhub.io/news/apple-m4.jpg",
            "related": "AAPL,TSM",
            "source": "MarketWatch",
            "summary": "Apple Inc. announced the rollout of its M4 chip family featuring industry-leading AI compute and memory bandwidth.",
            "url": "https://www.marketwatch.com/story/apple-m4-announcement-2024"
        },
        {
            "category": "general",
            "datetime": 1724658500,
            "headline": "NVIDIA Partners with Cloud Providers to Scale Blackwell GPU Deployments",
            "id": 7891235,
            "image": "https://img.finnhub.io/news/nvda-blackwell.jpg",
            "related": "NVDA,MSFT,AMZN",
            "source": "Reuters",
            "summary": "NVIDIA Corporation expands cloud enterprise partnerships to accelerate enterprise AI training.",
            "url": "https://www.reuters.com/technology/nvidia-blackwell-scale-2024"
        },
        {
            "category": "forex",
            "datetime": 1724659000,
            "headline": "Federal Reserve Signals Data-Dependent Stance Ahead of Jackson Hole Symposium",
            "id": 7891236,
            "image": "",
            "related": "SPY,QQQ",
            "source": "Bloomberg",
            "summary": "Fed officials emphasize balanced inflation and labor market metrics before adjusting policy rates.",
            "url": "https://www.bloomberg.com/news/fed-jackson-hole-2024"
        }
    ]"#;

    #[test]
    fn test_finnhub_article_json_parsing_and_raw_document_conversion() {
        let docs =
            parse_finnhub_news_json(SAMPLE_FINNHUB_JSON, 10).expect("Should parse valid JSON");
        assert_eq!(docs.len(), 3);

        // Verify Article 1 (AAPL)
        assert_eq!(docs[0].id, "finnhub-7891234");
        assert_eq!(
            docs[0].title,
            "Apple Unveils Next-Generation M4 Chip with Advanced Neural Engine for Mac"
        );
        assert_eq!(docs[0].source, "Finnhub-MarketWatch");
        assert_eq!(
            docs[0].url,
            "https://www.marketwatch.com/story/apple-m4-announcement-2024"
        );
        assert_eq!(docs[0].published_utc, "2024-08-26T07:40:00.000Z");
        assert!(docs[0].raw_content.contains("M4 chip family"));
        assert_eq!(docs[0].audio_path, None);

        // Verify Article 2 (NVDA)
        assert_eq!(docs[1].id, "finnhub-7891235");
        assert_eq!(
            docs[1].title,
            "NVIDIA Partners with Cloud Providers to Scale Blackwell GPU Deployments"
        );
        assert_eq!(docs[1].source, "Finnhub-Reuters");

        // Verify Article 3 (Macro)
        assert_eq!(docs[2].id, "finnhub-7891236");
        assert_eq!(docs[2].source, "Finnhub-Bloomberg");
    }

    #[test]
    fn test_finnhub_limit_parameter() {
        let docs_limited =
            parse_finnhub_news_json(SAMPLE_FINNHUB_JSON, 2).expect("Should limit results");
        assert_eq!(docs_limited.len(), 2);
    }

    #[test]
    fn test_finnhub_empty_json_returns_empty_vec() {
        let docs = parse_finnhub_news_json("[]", 10).expect("Should parse empty array");
        assert!(docs.is_empty());
    }

    #[test]
    fn test_finnhub_missing_optional_fields() {
        let partial_json = r#"[
            {
                "datetime": 1700000000,
                "headline": "Minimal Article With Defaulted Fields",
                "source": "",
                "summary": "Short snippet.",
                "url": "https://example.com/minimal"
            }
        ]"#;

        let docs = parse_finnhub_news_json(partial_json, 5).expect("Should parse partial JSON");
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].title, "Minimal Article With Defaulted Fields");
        assert_eq!(docs[0].source, "Finnhub"); // Fallback when source is empty
        assert_eq!(docs[0].raw_content, "Short snippet.");
    }

    static ENV_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_finnhub_client_from_env_lifecycle() {
        let _guard = ENV_MUTEX.lock().unwrap();
        env::remove_var("FINNHUB_API_KEY");
        let client_res = FinnhubClient::from_env();
        assert!(client_res.is_err());
        assert!(client_res.unwrap_err().contains("FINNHUB_API_KEY"));

        // Test placeholder key rejection
        env::set_var("FINNHUB_API_KEY", "your_finnhub_api_key_here");
        assert!(FinnhubClient::from_env().is_err());

        // Test valid key
        env::set_var("FINNHUB_API_KEY", "c12345testkeyabcdef67890");
        let client = FinnhubClient::from_env().expect("Should initialize from env");
        assert_eq!(client.api_key(), "c12345testkeyabcdef67890");
        env::remove_var("FINNHUB_API_KEY");
    }
}
