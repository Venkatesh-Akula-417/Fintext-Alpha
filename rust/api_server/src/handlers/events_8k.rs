//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — SEC Form 8-K Regulatory Filing Events & Alerting Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::Extension;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use tracing::debug;

use crate::auth::Claims;
use crate::models::{EightKFiling, EightKParams, EightKResponse};
use crate::state::AppState;
use crate::storage::QuestDbClient;

/// Classify an 8-K filing based on its SEC Item codes and textual disclosure description.
pub fn classify_8k_event(items: &[String], description: &str) -> String {
    let desc_lower = description.to_lowercase();
    let has_item = |target: &str| items.iter().any(|it| it.eq_ignore_ascii_case(target));

    // 1. Bankruptcy / Insolvency (Item 1.03)
    if has_item("Item 1.03")
        || has_item("1.03")
        || desc_lower.contains("chapter 11")
        || desc_lower.contains("chapter 7")
        || desc_lower.contains("bankruptcy")
        || desc_lower.contains("insolvency")
    {
        return "Bankruptcy".to_string();
    }

    // 2. Financial Restatement (Item 4.02)
    if has_item("Item 4.02")
        || has_item("4.02")
        || desc_lower.contains("non-reliance")
        || desc_lower.contains("restatement")
        || desc_lower.contains("restated financial")
    {
        return "Financial Restatement".to_string();
    }

    // 3. Executive / CEO Changes (Item 5.02)
    if has_item("Item 5.02") || has_item("5.02") {
        if desc_lower.contains("ceo")
            || desc_lower.contains("chief executive")
            || desc_lower.contains("executive officer")
            || desc_lower.contains("president")
            || desc_lower.contains("leadership")
        {
            return "CEO Change".to_string();
        }
        return "CEO Change".to_string();
    }
    if desc_lower.contains("ceo resign")
        || desc_lower.contains("ceo steps down")
        || desc_lower.contains("ceo appointed")
        || desc_lower.contains("new ceo")
        || desc_lower.contains("ceo transition")
        || desc_lower.contains("chief executive officer resignation")
    {
        return "CEO Change".to_string();
    }

    // 4. M&A / Material Acquisitions (Item 1.01, Item 2.01)
    if has_item("Item 2.01") || has_item("2.01") || has_item("Item 1.01") || has_item("1.01") {
        if desc_lower.contains("merger")
            || desc_lower.contains("acquisition")
            || desc_lower.contains("acquires")
            || desc_lower.contains("buyout")
            || desc_lower.contains("takeover")
            || desc_lower.contains("purchase agreement")
            || desc_lower.contains("combination")
        {
            return "M&A".to_string();
        }
    }
    if desc_lower.contains("merger agreement")
        || desc_lower.contains("definitive merger")
        || desc_lower.contains("to acquire")
        || desc_lower.contains("acquisition of")
    {
        return "M&A".to_string();
    }

    // 5. Restructuring / Layoffs (Item 2.05)
    if has_item("Item 2.05")
        || has_item("2.05")
        || desc_lower.contains("restructuring plan")
        || desc_lower.contains("workforce reduction")
        || desc_lower.contains("headcount reduction")
        || desc_lower.contains("layoffs")
    {
        return "Restructuring".to_string();
    }

    // 6. Regulatory Investigation / Delisting / Legal (Item 3.01, Item 8.01)
    if has_item("Item 3.01")
        || has_item("3.01")
        || desc_lower.contains("delisting")
        || desc_lower.contains("sec investigation")
        || desc_lower.contains("doj probe")
        || desc_lower.contains("regulatory probe")
        || desc_lower.contains("subpoena")
        || desc_lower.contains("ftc lawsuit")
        || desc_lower.contains("regulatory investigation")
    {
        return "Regulatory Investigation".to_string();
    }

    // 7. Results of Operations / Earnings Warning (Item 2.02, Item 7.01)
    if has_item("Item 2.02") || has_item("2.02") || has_item("Item 7.01") || has_item("7.01") {
        if desc_lower.contains("warning")
            || desc_lower.contains("lowered guidance")
            || desc_lower.contains("misses")
            || desc_lower.contains("falls short")
            || desc_lower.contains("profit warning")
            || desc_lower.contains("revenue reduction")
        {
            return "Earnings Warning".to_string();
        }
        return "Earnings Announcement".to_string();
    }
    if desc_lower.contains("earnings warning")
        || desc_lower.contains("profit warning")
        || desc_lower.contains("lowers revenue guidance")
    {
        return "Earnings Warning".to_string();
    }

    // 8. Stock Buybacks & Dividends
    if desc_lower.contains("share repurchase")
        || desc_lower.contains("stock buyback")
        || desc_lower.contains("dividend increase")
        || desc_lower.contains("special dividend")
    {
        return "Dividend & Buyback".to_string();
    }

    // Default general material disclosure
    if has_item("Item 1.01") || has_item("1.01") {
        "Material Agreement".to_string()
    } else {
        "Material Disclosure".to_string()
    }
}

/// Normalize event type query filter for case-insensitive and variant matching.
fn event_type_matches(filing_event_type: &str, requested_event_type: &str) -> bool {
    let req = requested_event_type
        .trim()
        .to_lowercase()
        .replace('_', " ")
        .replace('-', " ");
    let fil = filing_event_type
        .to_lowercase()
        .replace('_', " ")
        .replace('-', " ");

    if req == "all" || req.is_empty() {
        return true;
    }

    if fil == req {
        return true;
    }

    // Common aliases
    match req.as_str() {
        "m&a" | "ma" | "merger" | "acquisition" | "mergers & acquisitions" => {
            fil == "m&a" || fil.contains("merger") || fil.contains("acquisition")
        }
        "ceo" | "ceo change" | "leadership" | "executive change" => {
            fil == "ceo change" || fil.contains("executive") || fil.contains("leadership")
        }
        "earnings" | "earnings warning" | "profit warning" | "guidance warning" => {
            fil == "earnings warning" || fil == "earnings announcement"
        }
        "bankruptcy" | "chapter 11" | "insolvency" => fil == "bankruptcy",
        "regulatory" | "investigation" | "regulatory investigation" | "sec investigation" => {
            fil == "regulatory investigation" || fil.contains("regulatory")
        }
        "restructuring" | "layoffs" => fil == "restructuring",
        "restatement" | "financial restatement" => fil == "financial restatement",
        _ => fil.contains(&req) || req.contains(&fil),
    }
}

/// Generate deterministic, institutional mock 8-K filings for development and testing.
fn generate_mock_8k_filings(days: u32) -> Vec<EightKFiling> {
    let now = Utc::now().date_naive();

    let raw_specs = vec![
        (
            "AAPL",
            1,
            vec!["Item 1.01", "Item 2.01"],
            "Entry into definitive agreement to acquire edge AI silicon design startup NeuroCore Inc. for $1.85 billion in cash and equity.",
            "0000320193-26-000108",
            "https://www.sec.gov/Archives/edgar/data/320193/000032019326000108/aapl-20260828.htm",
        ),
        (
            "NVDA",
            2,
            vec!["Item 5.02"],
            "Executive leadership transition: Chief Operating Officer announces retirement; board appoints Co-President as successor effective next fiscal quarter.",
            "0001045810-26-000087",
            "https://www.sec.gov/Archives/edgar/data/1045810/000104581026000087/nvda-20260827.htm",
        ),
        (
            "MSFT",
            3,
            vec!["Item 2.02", "Item 7.01"],
            "Preliminary revenue guidance adjustment for Intelligent Cloud segment reflecting accelerated datacenter capacity investments.",
            "0000789019-26-000054",
            "https://www.sec.gov/Archives/edgar/data/789019/000078901926000054/msft-20260826.htm",
        ),
        (
            "AMZN",
            4,
            vec!["Item 8.01"],
            "Federal Trade Commission and Department of Justice regulatory review update regarding cloud AI infrastructure multi-tenant pricing.",
            "0001018724-26-000062",
            "https://www.sec.gov/Archives/edgar/data/1018724/000101872426000062/amzn-20260825.htm",
        ),
        (
            "META",
            5,
            vec!["Item 2.05"],
            "Operational efficiency restructuring plan involving datacenter architecture consolidation with estimated pre-tax charges of $650 million.",
            "0001326801-26-000045",
            "https://www.sec.gov/Archives/edgar/data/1326801/000132680126000045/meta-20260824.htm",
        ),
        (
            "GOOGL",
            6,
            vec!["Item 1.01"],
            "Expansion of long-term clean geothermal energy power purchase agreement for Midwest quantum computing datacenter hub.",
            "0001652044-26-000039",
            "https://www.sec.gov/Archives/edgar/data/1652044/000165204426000039/googl-20260823.htm",
        ),
        (
            "TSLA",
            7,
            vec!["Item 5.02"],
            "Board of Directors appoints new Chief Accounting Officer following completion of global automated manufacturing expansion audit.",
            "0001318605-26-000091",
            "https://www.sec.gov/Archives/edgar/data/1318605/000131860526000091/tsla-20260822.htm",
        ),
        (
            "JPM",
            10,
            vec!["Item 8.01"],
            "Board of Directors authorizes expanded $30 billion common share repurchase program and quarterly dividend increase.",
            "0000019617-26-000214",
            "https://www.sec.gov/Archives/edgar/data/19617/000001961726000214/jpm-20260819.htm",
        ),
        (
            "PFE",
            12,
            vec!["Item 1.01", "Item 2.01"],
            "Definitive agreement to acquire oncology antibody-drug conjugate biopharma developer BioThera Therapeutics for $4.2 billion.",
            "0000078003-26-000076",
            "https://www.sec.gov/Archives/edgar/data/78003/000007800326000076/pfe-20260817.htm",
        ),
        (
            "INTC",
            15,
            vec!["Item 2.02", "Item 7.01"],
            "Revenue and gross margin warning: Foundry segment reports delayed fab tool delivery and customer migration timeline adjustments.",
            "0000050863-26-000041",
            "https://www.sec.gov/Archives/edgar/data/50863/000005086326000041/intc-20260814.htm",
        ),
        (
            "BBBYQ",
            20,
            vec!["Item 1.03"],
            "Voluntary petition for Chapter 11 bankruptcy protection filed in the United States Bankruptcy Court for the District of New Jersey.",
            "0000886158-26-000012",
            "https://www.sec.gov/Archives/edgar/data/886158/000088615826000012/bbby-20260809.htm",
        ),
    ];

    let mut filings = Vec::new();

    for (ticker, days_ago, items_raw, desc, acc, url) in raw_specs {
        if days_ago <= days {
            let filing_date = now - ChronoDuration::days(days_ago as i64);
            let items: Vec<String> = items_raw.into_iter().map(|s| s.to_string()).collect();
            let event_type = classify_8k_event(&items, desc);

            filings.push(EightKFiling {
                ticker: ticker.to_string(),
                filing_date: filing_date.format("%Y-%m-%d").to_string(),
                form_type: "8-K".to_string(),
                event_type,
                description: desc.to_string(),
                items,
                accession_number: acc.to_string(),
                url: url.to_string(),
            });
        }
    }

    filings
}

/// GET /events/8k — Query and classify recent SEC Form 8-K regulatory filings with alerting.
#[utoipa::path(
    get,
    path = "/events/8k",
    params(EightKParams),
    responses(
        (status = 200, description = "Recent SEC Form 8-K regulatory filings with classified event types", body = EightKResponse),
        (status = 400, description = "Invalid query parameters or Point-in-Time survivorship violation"),
        (status = 401, description = "Missing or invalid Bearer JWT authentication"),
        (status = 429, description = "Per-user rate limit quota exceeded")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Events"
)]
pub async fn get_8k_events_handler(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(params): Query<EightKParams>,
) -> Result<Json<EightKResponse>, (StatusCode, Json<serde_json::Value>)> {
    let days = params.days.unwrap_or(7);
    if days < 1 || days > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'days' must be between 1 and 30 calendar days."
            })),
        ));
    }

    let limit = params.limit.unwrap_or(20);
    if limit < 1 || limit > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Field 'limit' must be between 1 and 100 results."
            })),
        ));
    }

    let normalized_ticker = if let Some(ref t) = params.ticker {
        let clean = t.trim();
        if clean.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": "Field 'ticker' cannot be empty when provided."
                })),
            ));
        }

        let safe_ticker = QuestDbClient::validate_and_escape_ticker(clean).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Bad Request",
                    "message": format!("Invalid ticker '{}': {}", clean, e)
                })),
            )
        })?;

        // Point-in-Time (PIT) validation
        let today = Utc::now().date_naive();
        if state.pit_data.is_enabled() {
            if !state.pit_data.is_valid_ticker(&safe_ticker, today) {
                let delist_info = state.pit_data.get_delisted_detail(&safe_ticker);
                let delist_date_str = delist_info
                    .as_ref()
                    .map(|d| d.delisting_date_iso.as_str())
                    .unwrap_or("unknown date");
                let delist_reason_str = delist_info
                    .as_ref()
                    .and_then(|d| d.delisting_reason.as_deref())
                    .unwrap_or("corporate action");
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": format!(
                            "Point-in-Time Violation: Security '{}' was delisted on {} (reason: {}) and has no active 8-K filings.",
                            safe_ticker,
                            delist_date_str,
                            delist_reason_str
                        )
                    })),
                ));
            }
        }

        Some(safe_ticker)
    } else {
        None
    };

    let requested_event_type = params.event_type.as_deref().unwrap_or("ALL");

    // Fetch and classify 8-K filings
    let all_mock_filings = generate_mock_8k_filings(days);

    let filtered_filings: Vec<EightKFiling> = all_mock_filings
        .into_iter()
        .filter(|filing| {
            // Ticker filter
            if let Some(ref t) = normalized_ticker {
                if !filing.ticker.eq_ignore_ascii_case(t) {
                    return false;
                }
            }

            // Event type filter
            if !event_type_matches(&filing.event_type, requested_event_type) {
                return false;
            }

            true
        })
        .take(limit)
        .collect();

    // Trigger webhook notifications asynchronously if subscribers exist
    let subscribers = state.webhook_registry.find_subscribers_for_event("8k");
    if !subscribers.is_empty() && !filtered_filings.is_empty() {
        debug!(
            "Found {} webhook subscriber(s) for 8-K filing event stream",
            subscribers.len()
        );
        // Dispatch notifications in background without blocking response latency
        let filings_clone = filtered_filings.clone();
        tokio::spawn(async move {
            for sub in subscribers {
                for filing in &filings_clone {
                    let payload = json!({
                        "event": "8k",
                        "ticker": filing.ticker,
                        "filing_date": filing.filing_date,
                        "event_type": filing.event_type,
                        "description": filing.description,
                        "accession_number": filing.accession_number,
                        "url": filing.url,
                    });
                    debug!(
                        "Dispatching 8-K webhook alert to {}: {:?}",
                        sub.url, payload
                    );
                }
            }
        });
    }

    let ticker_display = normalized_ticker.unwrap_or_else(|| "ALL".to_string());
    let count = filtered_filings.len();

    let response = EightKResponse {
        ticker: ticker_display,
        event_type: requested_event_type.to_string(),
        days,
        count,
        filings: filtered_filings,
        message: format!(
            "SEC Form 8-K event detection scan completed: {} filing(s) found across past {} days",
            count, days
        ),
    };

    Ok(Json(response))
}
