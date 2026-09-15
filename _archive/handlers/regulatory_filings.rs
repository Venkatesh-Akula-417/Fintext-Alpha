//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — SEC Regulatory Filings Classifier & Discovery Engine
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides standardized classification and retrieval across all major SEC EDGAR
//! filing types (10-K, 10-Q, 8-K, 20-F, 6-K, S-1, DEF 14A, Form 4, etc.) mapping
//! submissions to institutional event categories.
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::models::{RegulatoryFilingItem, RegulatoryFilingsParams, RegulatoryFilingsResponse};
use crate::pit::GLOBAL_PIT_DATA;
use crate::state::AppState;
use crate::storage::questdb_client::QuestDbClient;

/// Standard institutional event categories for SEC regulatory filings.
pub const VALID_EVENT_CATEGORIES: &[&str] = &[
    "Annual Report",
    "Quarterly Report",
    "Registration Statement",
    "M&A",
    "Bankruptcy",
    "Earnings",
    "Governance",
    "Insider Transaction",
    "Proxy Statement",
    "Foreign Issuer Report",
    "Financial Restatement",
    "Restructuring",
    "Regulatory Investigation",
    "Other",
];

/// Classify an SEC filing into a standardized institutional event category.
pub fn classify_filing(form_type: &str, description: &str) -> String {
    let form_clean = form_type.trim().to_uppercase();
    let desc_lower = description.to_lowercase();

    match form_clean.as_str() {
        "10-K" | "10-K/A" | "10-KT" | "10-KT/A" => "Annual Report".to_string(),
        "10-Q" | "10-Q/A" | "10-QT" | "10-QT/A" => "Quarterly Report".to_string(),
        "20-F" | "20-F/A" | "40-F" | "40-F/A" => "Foreign Issuer Report".to_string(),
        "S-1" | "S-1/A" | "S-3" | "S-3/A" | "S-8" | "S-8/A" => "Registration Statement".to_string(),
        "S-4" | "S-4/A" => {
            if desc_lower.contains("merger")
                || desc_lower.contains("acquisition")
                || desc_lower.contains("business combination")
            {
                "M&A".to_string()
            } else {
                "Registration Statement".to_string()
            }
        }
        "DEF 14A" | "DEFA14A" | "PRE 14A" | "PREM14A" => {
            if desc_lower.contains("merger") || desc_lower.contains("acquisition") {
                "M&A".to_string()
            } else {
                "Proxy Statement".to_string()
            }
        }
        "4" | "FORM 4" | "3" | "FORM 3" | "5" | "FORM 5" => "Insider Transaction".to_string(),
        "8-K" | "8-K/A" | "6-K" | "6-K/A" => {
            if desc_lower.contains("chapter 11")
                || desc_lower.contains("chapter 7")
                || desc_lower.contains("bankruptcy")
                || desc_lower.contains("insolvency")
            {
                "Bankruptcy".to_string()
            } else if desc_lower.contains("restatement")
                || desc_lower.contains("non-reliance")
                || desc_lower.contains("restated financial")
            {
                "Financial Restatement".to_string()
            } else if desc_lower.contains("merger")
                || desc_lower.contains("acquisition")
                || desc_lower.contains("acquires")
                || desc_lower.contains("buyout")
                || desc_lower.contains("takeover")
                || desc_lower.contains("purchase agreement")
            {
                "M&A".to_string()
            } else if desc_lower.contains("ceo")
                || desc_lower.contains("chief executive")
                || desc_lower.contains("director")
                || desc_lower.contains("governance")
                || desc_lower.contains("officer")
                || desc_lower.contains("appointed")
                || desc_lower.contains("resignation")
            {
                "Governance".to_string()
            } else if desc_lower.contains("restructuring")
                || desc_lower.contains("workforce reduction")
                || desc_lower.contains("layoffs")
            {
                "Restructuring".to_string()
            } else if desc_lower.contains("investigation")
                || desc_lower.contains("subpoena")
                || desc_lower.contains("sec probe")
                || desc_lower.contains("doj probe")
                || desc_lower.contains("ftc lawsuit")
                || desc_lower.contains("regulatory probe")
                || desc_lower.contains("delisting")
            {
                "Regulatory Investigation".to_string()
            } else if desc_lower.contains("earnings")
                || desc_lower.contains("quarterly results")
                || desc_lower.contains("financial results")
                || desc_lower.contains("guidance")
                || desc_lower.contains("revenue")
            {
                "Earnings".to_string()
            } else if form_clean.starts_with("6-K") {
                "Foreign Issuer Report".to_string()
            } else {
                "Other".to_string()
            }
        }
        _ => {
            if desc_lower.contains("bankruptcy") {
                "Bankruptcy".to_string()
            } else if desc_lower.contains("merger") || desc_lower.contains("acquisition") {
                "M&A".to_string()
            } else if desc_lower.contains("earnings") {
                "Earnings".to_string()
            } else if desc_lower.contains("annual report") {
                "Annual Report".to_string()
            } else if desc_lower.contains("quarterly report") {
                "Quarterly Report".to_string()
            } else {
                "Other".to_string()
            }
        }
    }
}

/// Generates mock regulatory filings for a given ticker and date range.
pub fn generate_mock_filings_for_ticker(
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<RegulatoryFilingItem> {
    let clean = ticker.trim().to_uppercase();
    let mut hasher = DefaultHasher::new();
    clean.hash(&mut hasher);
    let seed = hasher.finish();

    let mut items = Vec::new();
    let days_span = (end_date - start_date).num_days().max(1) as u64;

    let templates: &[(&str, &str)] = &[
        ("10-K", "Annual Report for Fiscal Year Ended December 31, 2025 pursuant to Section 13 or 15(d)"),
        ("10-Q", "Quarterly Report for the Period Ended June 30, 2025 containing unaudited financial statements"),
        ("8-K", "Item 1.01 Entry into a Material Definitive Purchase and Merger Agreement to acquire AI infrastructure provider"),
        ("8-K", "Item 2.02 Results of Operations and Financial Condition for Q3 2025 and updated full-year earnings guidance"),
        ("8-K", "Item 5.02 Departure of Directors or Principal Officers; Election of Directors; Appointment of Chief Operating Officer"),
        ("S-1", "Registration Statement under the Securities Act of 1933 for Secondary Offering of Common Stock"),
        ("DEF 14A", "Notice of Annual Meeting of Shareholders and Definitive Proxy Statement regarding executive compensation and board elections"),
        ("Form 4", "Statement of Changes in Beneficial Ownership of Securities by Chief Technology Officer"),
    ];

    for (idx, &(form, desc)) in templates.iter().enumerate() {
        let day_offset = (seed.wrapping_add(idx as u64 * 17)) % days_span;
        let filing_date = start_date + ChronoDuration::days(day_offset as i64);

        if filing_date < start_date || filing_date > end_date {
            continue;
        }

        let cik = match clean.as_str() {
            "AAPL" => "0000320193",
            "NVDA" => "0001045810",
            "MSFT" => "0000789019",
            "AMZN" => "0001018724",
            "GOOGL" => "0001652044",
            "META" => "0001326801",
            "TSLA" => "0001318605",
            "PYPL" => "0001633917",
            _ => "0000999999",
        };

        let accession = format!(
            "{}-{:02}-{:06}",
            cik,
            (filing_date.format("%y").to_string()),
            (seed.wrapping_add(idx as u64)) % 900000 + 100000
        );
        let url = format!(
            "https://www.sec.gov/ix?doc=/Archives/edgar/data/{}/{}/primary-document.htm",
            cik,
            accession.replace('-', "")
        );
        let event_category = classify_filing(form, desc);

        items.push(RegulatoryFilingItem {
            ticker: clean.clone(),
            form_type: form.to_string(),
            filing_date: filing_date.format("%Y-%m-%d").to_string(),
            accession_number: accession,
            event_category,
            description: desc.to_string(),
            url,
        });
    }

    items
}

/// Handler for `GET /events/filings`
#[utoipa::path(
    get,
    path = "/events/filings",
    params(
        RegulatoryFilingsParams
    ),
    responses(
        (status = 200, description = "SEC regulatory filings retrieved and classified successfully", body = RegulatoryFilingsResponse),
        (status = 400, description = "Invalid query parameters (bounds, dates, or invalid filters)"),
        (status = 401, description = "Missing or invalid Bearer JWT / API Key authentication"),
        (status = 429, description = "Rate limit capacity exceeded")
    ),
    security(
        ("bearerAuth" = [])
    ),
    tag = "Events & Catalysts"
)]
pub async fn get_regulatory_filings_handler(
    State(_state): State<AppState>,
    Query(params): Query<RegulatoryFilingsParams>,
) -> Result<Json<RegulatoryFilingsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let today = Utc::now().date_naive();

    // 1. Date window parsing & validation
    let end_date = if let Some(ref end_str) = params.end_date {
        NaiveDate::parse_from_str(end_str.trim(), "%Y-%m-%d").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Invalid end_date format: '{}'. Expected ISO YYYY-MM-DD", end_str),
                    "field": "end_date"
                })),
            )
        })?
    } else {
        today
    };

    let start_date = if let Some(ref start_str) = params.start_date {
        NaiveDate::parse_from_str(start_str.trim(), "%Y-%m-%d").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Invalid start_date format: '{}'. Expected ISO YYYY-MM-DD", start_str),
                    "field": "start_date"
                })),
            )
        })?
    } else {
        end_date - ChronoDuration::days(90)
    };

    if start_date > end_date {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("start_date ({}) cannot be later than end_date ({})", start_date, end_date),
                "field": "start_date"
            })),
        ));
    }

    // 2. Pagination bounds validation
    let limit = params.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Invalid Parameter",
                "message": format!("limit must be between 1 and 100, got {}", limit),
                "field": "limit"
            })),
        ));
    }

    let offset = params.offset.unwrap_or(0);

    // 3. Ticker filtering & Point-in-Time check
    let mut candidate_tickers = Vec::new();
    let filter_ticker = if let Some(ref raw_t) = params.ticker {
        let clean = QuestDbClient::validate_and_escape_ticker(raw_t).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid Parameter",
                    "message": format!("Validation failed for field 'ticker': {}", e),
                    "field": "ticker"
                })),
            )
        })?;

        // Point-in-Time check for specific ticker
        if !GLOBAL_PIT_DATA.is_valid_ticker(&clean, start_date) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Point-in-Time Security Invalidation",
                    "message": format!("Ticker '{}' was not active or listed on '{}' (Point-in-Time check failed)", clean, start_date),
                    "ticker": clean
                })),
            ));
        }

        candidate_tickers.push(clean.clone());
        Some(clean)
    } else {
        let tracked_universe = [
            "AAPL", "NVDA", "MSFT", "AMZN", "GOOGL", "META", "TSLA", "AMD", "INTC", "CRM", "AVGO",
            "QCOM", "TXN", "PYPL", "ADBE", "NFLX", "CSCO", "ORCL", "IBM", "NOW",
        ];
        for &t in &tracked_universe {
            if GLOBAL_PIT_DATA.is_valid_ticker(t, start_date) {
                candidate_tickers.push(t.to_string());
            }
        }
        None
    };

    // 4. Gather & filter filings
    let mut all_filings = Vec::new();
    for t in candidate_tickers {
        let filings = generate_mock_filings_for_ticker(&t, start_date, end_date);
        all_filings.extend(filings);
    }

    // Filter by form_type
    if let Some(ref ft) = params.form_type {
        let clean_ft = ft.trim().to_uppercase();
        all_filings.retain(|f| f.form_type.to_uppercase() == clean_ft);
    }

    // Filter by event_category
    if let Some(ref ec) = params.event_category {
        let clean_ec = ec.trim().to_lowercase();
        all_filings.retain(|f| f.event_category.to_lowercase() == clean_ec);
    }

    // Sort by filing_date descending
    all_filings.sort_by(|a, b| b.filing_date.cmp(&a.filing_date));

    let total_count = all_filings.len();

    // Apply pagination
    let paginated_filings: Vec<RegulatoryFilingItem> =
        all_filings.into_iter().skip(offset).take(limit).collect();

    let count = paginated_filings.len();

    Ok(Json(RegulatoryFilingsResponse {
        ticker: filter_ticker,
        form_type: params.form_type.map(|s| s.trim().to_uppercase()),
        event_category: params.event_category.map(|s| s.trim().to_string()),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        count,
        total_count,
        offset,
        limit,
        filings: paginated_filings,
        generated_at: Utc::now().to_rfc3339(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_filing_forms() {
        assert_eq!(classify_filing("10-K", "Annual Report"), "Annual Report");
        assert_eq!(
            classify_filing("10-Q", "Quarterly Report"),
            "Quarterly Report"
        );
        assert_eq!(
            classify_filing("20-F", "Foreign Private Issuer Annual Report"),
            "Foreign Issuer Report"
        );
        assert_eq!(
            classify_filing("S-1", "Registration Statement"),
            "Registration Statement"
        );
        assert_eq!(
            classify_filing("DEF 14A", "Proxy Statement"),
            "Proxy Statement"
        );
        assert_eq!(
            classify_filing("Form 4", "Insider statement"),
            "Insider Transaction"
        );
    }

    #[test]
    fn test_classify_filing_8k_disclosures() {
        assert_eq!(
            classify_filing("8-K", "Definitive merger agreement to acquire partner"),
            "M&A"
        );
        assert_eq!(
            classify_filing("8-K", "Company files Chapter 11 bankruptcy petition"),
            "Bankruptcy"
        );
        assert_eq!(
            classify_filing("8-K", "Appointment of new Chief Executive Officer"),
            "Governance"
        );
        assert_eq!(
            classify_filing("8-K", "Q3 Financial Results and Earnings Release"),
            "Earnings"
        );
        assert_eq!(
            classify_filing("8-K", "Notice of Restructuring and Workforce Reduction"),
            "Restructuring"
        );
    }
}
