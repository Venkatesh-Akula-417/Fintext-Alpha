//! Prometheus client and Webhook alert dispatcher.

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::detector::AnomalyReport;

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: Option<PrometheusData>,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    #[allow(dead_code)]
    #[serde(rename = "resultType")]
    result_type: Option<String>,
    result: Vec<PrometheusResultItem>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResultItem {
    value: Option<(serde_json::Value, String)>,
}

pub struct ObservabilityClient {
    client: Client,
    prometheus_url: String,
    webhook_url: Option<String>,
}

impl ObservabilityClient {
    pub fn new(prometheus_url: String, webhook_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self {
            client,
            prometheus_url,
            webhook_url,
        }
    }

    /// Query Prometheus instant query endpoint and parse the float value.
    pub async fn query_metric(&self, query: &str) -> Result<f64, String> {
        let endpoint = format!("{}/api/v1/query", self.prometheus_url.trim_end_matches('/'));
        let res = self
            .client
            .get(&endpoint)
            .query(&[("query", query)])
            .send()
            .await
            .map_err(|e| format!("Prometheus request failed: {}", e))?;

        if !res.status().is_success() {
            return Err(format!(
                "Prometheus returned non-200 status: {}",
                res.status()
            ));
        }

        let body: PrometheusResponse = res
            .json()
            .await
            .map_err(|e| format!("Failed to parse Prometheus JSON: {}", e))?;

        if body.status != "success" {
            return Err(format!("Prometheus returned status: {}", body.status));
        }

        let data = body
            .data
            .ok_or_else(|| "Missing data in Prometheus response".to_string())?;
        if data.result.is_empty() {
            return Err("Prometheus query returned empty result set".to_string());
        }

        let item = &data.result[0];
        if let Some((_, ref val_str)) = item.value {
            val_str
                .parse::<f64>()
                .map_err(|e| format!("Failed to parse metric float '{}': {}", val_str, e))
        } else {
            Err("No scalar value present in Prometheus result item".to_string())
        }
    }

    /// Dispatch webhook alert to notify external auto-scaler / rollback controller.
    pub async fn trigger_webhook(&self, report: &AnomalyReport) -> Result<(), String> {
        let url = match &self.webhook_url {
            Some(u) => u,
            None => {
                info!("No WEBHOOK_URL configured. Skipping webhook dispatch.");
                return Ok(());
            }
        };

        let payload = serde_json::json!({
            "event": "LATENCY_ANOMALY_DETECTED",
            "source": "fintext_anomaly_detector",
            "anomaly_report": report,
            "recommended_action": "TRIGGER_CANARY_ROLLBACK_OR_AUTOSCALE"
        });

        match self.client.post(url).json(&payload).send().await {
            Ok(res) => {
                if res.status().is_success() {
                    info!(
                        webhook_url = %url,
                        status = %res.status(),
                        "Successfully triggered anomaly rollback/scaling webhook."
                    );
                    Ok(())
                } else {
                    warn!(
                        webhook_url = %url,
                        status = %res.status(),
                        "Webhook endpoint returned non-success status."
                    );
                    Ok(())
                }
            }
            Err(e) => {
                error!(webhook_url = %url, error = %e, "Failed to send webhook notification.");
                Err(format!("Webhook POST failed: {}", e))
            }
        }
    }
}
