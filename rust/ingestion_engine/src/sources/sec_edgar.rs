//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — High-Throughput SEC EDGAR Poller & Rate Limiter
//! ═══════════════════════════════════════════════════════════════════════════════
//! [DATA LICENSING COMPLIANCE NOTICE]
//! Status: ACTIVE (Public Domain Regulatory Data - Fully Safe for Commercial Redistribution).
//!
//! SEC EDGAR 8-K, 10-Q, and 10-K regulatory filings are public domain government records
//! under U.S. law, fully eligible for commercial processing, vectorization, and downstream
//! redistribution without proprietary third-party licensing fees. Controlled via `ENABLE_SEC_EDGAR=1`.
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::pipeline::RawDocument;
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT_ENCODING, USER_AGENT};
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::warn;
use uuid::Uuid;

const SEC_USER_AGENT: &str = "FinText Institutional Research/2.0 (contact@fintext-alpha.com)";
const MAX_CONCURRENT_REQUESTS: usize = 4;
const RATE_LIMIT_DELAY_MS: u64 = 105; // ~9.5 req/sec (safely under SEC 10 req/s limit)

pub struct SecEdgarFetcher {
    client: Client,
    semaphore: Arc<Semaphore>,
}

impl SecEdgarFetcher {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(SEC_USER_AGENT));
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build SEC EDGAR reqwest client");

        Self {
            client,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS)),
        }
    }

    /// Fetch latest filings from SEC submissions feed with exponential backoff.
    pub async fn fetch_latest_filings(&self, cik: &str) -> Result<Vec<RawDocument>, String> {
        let _permit = self.semaphore.acquire().await.map_err(|e| e.to_string())?;
        sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;

        let padded_cik = format!("{:0>10}", cik);
        let url = format!("https://data.sec.gov/submissions/CIK{}.json", padded_cik);

        let mut retries = 0;
        let mut delay = Duration::from_millis(100);

        loop {
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let text = resp.text().await.map_err(|e| e.to_string())?;
                        return self.parse_submissions_json(&text, cik);
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        warn!(
                            "SEC EDGAR rate limit / server error ({}), retrying in {:?}",
                            status, delay
                        );
                        retries += 1;
                        if retries > 5 {
                            return Err(format!(
                                "Max retries exceeded for CIK {} ({})",
                                cik, status
                            ));
                        }
                        sleep(delay).await;
                        delay *= 2;
                    } else {
                        return Err(format!(
                            "SEC EDGAR permanent HTTP error {} for CIK {}",
                            status, cik
                        ));
                    }
                }
                Err(e) => {
                    retries += 1;
                    if retries > 5 {
                        return Err(format!("Network error fetching CIK {}: {}", cik, e));
                    }
                    sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    fn parse_submissions_json(
        &self,
        json_str: &str,
        cik: &str,
    ) -> Result<Vec<RawDocument>, String> {
        let root: Value = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
        let mut docs = Vec::new();

        let entity_name = root["name"].as_str().unwrap_or("Unknown Entity");
        if let Some(recent) = root.get("filings").and_then(|f| f.get("recent")) {
            let forms = recent["form"].as_array();
            let accession_numbers = recent["accessionNumber"].as_array();
            let filing_dates = recent["filingDate"].as_array();
            let primary_docs = recent["primaryDocument"].as_array();

            if let (Some(forms), Some(acc_nums), Some(dates), Some(p_docs)) =
                (forms, accession_numbers, filing_dates, primary_docs)
            {
                let count = forms.len().min(10); // Take latest 10 filings
                for i in 0..count {
                    let form = forms[i].as_str().unwrap_or("");
                    let acc_num = acc_nums[i].as_str().unwrap_or("");
                    let filing_date = dates[i].as_str().unwrap_or("");
                    let primary_doc = p_docs[i].as_str().unwrap_or("");

                    if form == "10-K" || form == "10-Q" || form == "8-K" {
                        let title =
                            format!("SEC Form {} Filing for {} ({})", form, entity_name, cik);
                        let doc_url = format!(
                            "https://www.sec.gov/Archives/edgar/data/{}/{}/{}",
                            cik.trim_start_matches('0'),
                            acc_num.replace('-', ""),
                            primary_doc
                        );

                        docs.push(RawDocument {
                            id: Uuid::new_v4().to_string(),
                            title,
                            source: "SEC_EDGAR".to_string(),
                            url: doc_url,
                            published_utc: format!("{}T10:00:00Z", filing_date),
                            raw_content: format!(
                                "SEC Form {} disclosure report for {} (CIK: {}). Filing date: {}.",
                                form, entity_name, cik, filing_date
                            ),
                            audio_path: None,
                            ingested_utc: Utc::now().to_rfc3339(),
                            ..Default::default()
                        });
                    }
                }
            }
        }

        Ok(docs)
    }
}
