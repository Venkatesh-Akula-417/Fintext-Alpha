//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Unified Cross-Domain Search API Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::auth::Claims;
use crate::models::{
    ListNewsArticlesQuery, SearchParams, SearchResponse, SearchResultItem, TranscriptListParams,
};
use crate::state::AppState;

/// Allowed searchable data domains.
pub const VALID_SEARCH_TYPES: &[&str] = &[
    "news",
    "sentiment",
    "transcripts",
    "filings",
    "events",
    "insider",
    "supply_chain",
    "options",
];

/// Internal searchable item representation before relevance scoring.
#[derive(Debug, Clone)]
pub struct SearchableItem {
    pub id: String,
    pub item_type: String,
    pub ticker: String,
    pub title: String,
    pub body: String,
    pub date: String,
}

/// Computes relevance ranking score for a query against a searchable item.
///
/// Scoring Hierarchy:
/// 1. Exact ticker match (case-insensitive): 1.00
/// 2. Substring in ticker OR Substring in title (case-insensitive): 0.80 (+ 0.05 bonus if prefix/exact match)
/// 3. Substring in body / snippet (case-insensitive): 0.60
/// 4. No match: 0.00 (excluded)
pub fn compute_relevance_score(query: &str, ticker: &str, title: &str, body: &str) -> f64 {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return 0.0;
    }

    let ticker_lower = ticker.trim().to_lowercase();
    let title_lower = title.trim().to_lowercase();
    let body_lower = body.trim().to_lowercase();

    // 1. Exact ticker match
    if ticker_lower == q {
        return 1.0;
    }

    // 2. Substring in ticker OR Substring in title
    if ticker_lower.contains(&q) || title_lower.contains(&q) {
        let bonus: f64 = if title_lower.starts_with(&q) || title_lower == q {
            0.05
        } else {
            0.0
        };
        return (0.80_f64 + bonus).min(1.0_f64);
    }

    // 3. Substring in body / snippet
    if body_lower.contains(&q) {
        return 0.60;
    }

    0.0
}

/// Checks if an item date matches start_date and end_date filters.
pub fn matches_date_filter(
    item_date: &str,
    start_date: Option<&str>,
    end_date: Option<&str>,
) -> bool {
    let item_prefix = if item_date.len() >= 10 {
        &item_date[..10]
    } else {
        item_date
    };

    if let Some(start) = start_date {
        let start_prefix = if start.len() >= 10 {
            &start[..10]
        } else {
            start
        };
        if item_prefix < start_prefix {
            return false;
        }
    }

    if let Some(end) = end_date {
        let end_prefix = if end.len() >= 10 { &end[..10] } else { end };
        if item_prefix > end_prefix {
            return false;
        }
    }

    true
}

/// Truncates string to a maximum number of characters for snippets.
fn truncate_snippet(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let snippet: String = trimmed.chars().take(max_chars).collect();
        format!("{}...", snippet.trim_end())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cross-Domain Search Collectors
// ─────────────────────────────────────────────────────────────────────────────

fn collect_news_items(state: &AppState) -> Vec<SearchableItem> {
    let mut items = Vec::new();
    let query = ListNewsArticlesQuery {
        ticker: None,
        source: None,
        start_date: None,
        end_date: None,
        limit: Some(100),
        offset: Some(0),
    };
    let registry_articles = state.news_article_registry.list_articles(&query);

    for meta in registry_articles.articles {
        let full_text = state
            .news_article_registry
            .get_article(&meta.id)
            .map(|a| a.full_text)
            .unwrap_or_else(|| meta.snippet.clone());

        items.push(SearchableItem {
            id: meta.id.to_string(),
            item_type: "news".to_string(),
            ticker: meta.ticker,
            title: meta.title,
            body: full_text,
            date: meta.published_utc.to_rfc3339(),
        });
    }

    // Built-in baseline news items
    let base_news = [
        ("AAPL", "Apple reports record quarterly revenue driven by AI iPhone demand", "Apple Inc. today announced financial results for its fiscal 2025 fourth quarter. The Company posted quarterly revenue of $94.9 billion, up 6 percent year over year.", "2025-08-15T10:00:00Z"),
        ("NVDA", "NVIDIA announces next-generation Blackwell Ultra GPU architecture", "NVIDIA today introduced its Blackwell Ultra AI supercomputing platform, delivering 4x higher inference performance for trillion-parameter LLMs across tier-1 hyperscale datacenters.", "2025-08-20T14:30:00Z"),
        ("MSFT", "Microsoft expands Azure AI infrastructure with nuclear energy partnership", "Microsoft Corp. announced a major 20-year power purchase agreement to power its next-generation Intelligent Cloud AI datacenter clusters with carbon-free clean energy.", "2025-08-22T09:15:00Z"),
        ("TSLA", "Tesla achieves autonomous robotaxi deployment milestone across five metro hubs", "Tesla Inc. deployed its full unsupervised Full Self-Driving (FSD) commercial robotaxi ride-hailing network in Austin, Miami, and Phoenix following regulatory safety approvals.", "2025-08-25T16:00:00Z"),
        ("GOOGL", "Google DeepMind unveils Gemini 2.5 multimodal reasoning benchmarks", "Google DeepMind researchers released Gemini 2.5 Flash and Pro models featuring native audio, vision, and ultra-long 4M token context window capabilities for enterprise quants.", "2025-08-26T11:45:00Z"),
        ("AMZN", "Amazon Web Services launches Graviton5 and Trainium3 silicon chips", "AWS announced the general availability of custom designed Trainium3 generative AI accelerators offering 50% lower cost-to-train for financial language vectorization models.", "2025-08-27T13:20:00Z"),
        ("META", "Meta releases open-source Llama 4 frontier AI weights and quantitative models", "Meta Platforms published open-weights for Llama 4 405B with optimized tensor parallel inference kernels and fine-tuned financial reasoning checkpoints.", "2025-08-28T08:00:00Z"),
        ("JPM", "JPMorgan Chase reports record institutional trading and advisory revenue", "JPMorgan Chase & Co. posted quarterly investment banking fees of $2.4 billion, reflecting surging quantitative equity trading volumes and M&A advisory activity.", "2025-08-29T12:00:00Z"),
    ];

    for (ticker, title, body, date) in base_news {
        items.push(SearchableItem {
            id: format!("news_{}", Uuid::new_v4()),
            item_type: "news".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        });
    }

    items
}

fn collect_transcript_items(state: &AppState, user_id: &str) -> Vec<SearchableItem> {
    let mut items = Vec::new();
    let params = TranscriptListParams {
        ticker: None,
        quarter: None,
        year: None,
        start_date: None,
        end_date: None,
        limit: Some(100),
        offset: Some(0),
    };
    let user_transcripts = state.transcript_registry.list(user_id, &params);

    for t in user_transcripts.items {
        let title = format!(
            "{} Q{} {} Earnings Call Transcript",
            t.ticker,
            t.quarter.unwrap_or(1),
            t.year.unwrap_or(2025)
        );
        let body = state
            .transcript_registry
            .get(user_id, t.id)
            .map(|full| full.transcript_text)
            .unwrap_or_else(|| title.clone());

        items.push(SearchableItem {
            id: t.id.to_string(),
            item_type: "transcript".to_string(),
            ticker: t.ticker,
            title,
            body,
            date: t.call_date.unwrap_or_else(|| "2025-08-01".to_string()),
        });
    }

    let base_transcripts = [
        ("AAPL", "Apple Inc. Q3 2025 Earnings Call Transcript", "Tim Cook: Good afternoon everyone and thank you for joining us. We are pleased to report strong performance across all our product categories, driven by customer enthusiasm for Apple Intelligence features.", "2025-07-31"),
        ("NVDA", "NVIDIA Corp. Q2 2025 Earnings Conference Call Transcript", "Jensen Huang: Accelerated computing and generative AI have hit the tipping point. Demand is surging worldwide across companies, industries and nations.", "2025-08-27"),
        ("MSFT", "Microsoft Corp. Q4 2025 Financial Results Conference Call", "Satya Nadella: As a company, we are focused on helping customers use our AI platforms and tools to drive operational efficiency and create new growth opportunities.", "2025-07-29"),
        ("AMZN", "Amazon.com Inc. Q2 2025 Earnings Call Transcript", "Andy Jassy: We continue to see robust growth in AWS as enterprise customers accelerate their migration to cloud and leverage our Bedrock generative AI services.", "2025-08-01"),
        ("TSLA", "Tesla Inc. Q2 2025 Financial Update Call Transcript", "Elon Musk: We made substantial progress with our next-generation vehicle platform and autonomous vehicle fleet scaling during the second quarter.", "2025-07-23"),
    ];

    for (ticker, title, body, date) in base_transcripts {
        items.push(SearchableItem {
            id: format!("transcript_{}", Uuid::new_v4()),
            item_type: "transcript".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        });
    }

    items
}

fn collect_sentiment_items() -> Vec<SearchableItem> {
    let base_sentiment = [
        ("AAPL", "Apple Sentiment Analysis: Bullish Momentum (0.78)", "Natural language sentiment score of 0.78 with 88% model confidence based on institutional filings and analyst commentary.", "2025-08-29T12:00:00Z"),
        ("NVDA", "NVIDIA Sentiment Breakdown: Very Bullish (0.92)", "Strong positive sentiment velocity following datacenter sales expansion and supply chain yield improvements.", "2025-08-29T12:00:00Z"),
        ("MSFT", "Microsoft Market Sentiment: Moderately Bullish (0.64)", "Consistent enterprise cloud migration sentiment offset by elevated capital expenditure guidance.", "2025-08-29T12:00:00Z"),
        ("TSLA", "Tesla Sentiment Dispersion: Neutral-Bullish (0.52)", "High analyst sentiment disagreement with wide options volatility skew following autonomous vehicle testing updates.", "2025-08-29T12:00:00Z"),
        ("AMZN", "Amazon Sentiment Outlook: Bullish (0.74)", "E-commerce margin expansion and AWS AI adoption generate strong quantitative buy signals across multi-asset funds.", "2025-08-29T12:00:00Z"),
        ("GOOGL", "Alphabet Sentiment Indicator: Bullish (0.71)", "Search advertising stability and cloud profitability generate positive sentiment breadth across quant universes.", "2025-08-29T12:00:00Z"),
    ];

    base_sentiment
        .into_iter()
        .map(|(ticker, title, body, date)| SearchableItem {
            id: format!("sentiment_{}", Uuid::new_v4()),
            item_type: "sentiment".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

fn collect_filings_items() -> Vec<SearchableItem> {
    let base_filings = [
        ("AAPL", "SEC Form 10-Q Quarterly Report (Q3 2025)", "Quarterly report pursuant to Section 13 or 15(d) of the Securities Exchange Act of 1934 for the period ended June 28, 2025.", "2025-08-01"),
        ("NVDA", "SEC Form 8-K Unscheduled Material Event: Leadership Appointment", "Item 5.02: Departure of Directors or Certain Officers; Election of Directors; Appointment of Certain Officers.", "2025-08-27"),
        ("MSFT", "SEC Form 10-K Annual Report (Fiscal Year 2025)", "Annual report pursuant to Section 13 or 15(d) of the Securities Exchange Act of 1934 for the fiscal year ended June 30, 2025.", "2025-07-30"),
        ("AMZN", "SEC Form 8-K Regulatory Compliance Update", "Item 8.01 Other Events: Federal Trade Commission review resolution regarding cloud computing multi-tenant pricing.", "2025-08-25"),
        ("TSLA", "SEC Form 8-K Executive Officer Transition", "Item 5.02 Appointment of Chief Accounting Officer and update on global manufacturing expansion milestones.", "2025-08-22"),
        ("JPM", "SEC Form 8-K Capital Management & Share Repurchase", "Item 8.01 Board of Directors authorizes $30 billion share buyback program and dividend increase.", "2025-08-19"),
        ("PFE", "SEC Form 8-K Material Definitive Agreement: Oncology Acquisition", "Item 1.01 Entry into a Material Definitive Agreement to acquire BioThera Therapeutics for $4.2 billion.", "2025-08-17"),
        ("BBBYQ", "SEC Form 8-K Bankruptcy Filing (Item 1.03)", "Item 1.03 Bankruptcy or Receivership: Voluntary petition under Chapter 11 filed in the District of New Jersey.", "2025-08-09"),
    ];

    base_filings
        .into_iter()
        .map(|(ticker, title, body, date)| SearchableItem {
            id: format!("filing_{}", Uuid::new_v4()),
            item_type: "filing".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

fn collect_events_items() -> Vec<SearchableItem> {
    let base_events = [
        ("AAPL", "Earnings Surprise Event: +8.4% EPS Beat", "Apple reported quarterly EPS of $1.64 vs consensus estimate of $1.51, triggering +3.2% cumulative abnormal return.", "2025-08-01"),
        ("NVDA", "M&A Catalyst Rumor: Optical Interconnect Technology Acquisition", "Multi-signal rumor score 84.5%: NVIDIA in advanced discussions to acquire Silicon Photonic Systems for AI networking.", "2025-08-26"),
        ("MSFT", "Cumulative Abnormal Return (CAR) Event Study: Clean Energy Deal", "Event study window [-2, +2] shows +2.15% abnormal return and elevated institutional volume ratio.", "2025-08-23"),
        ("TSLA", "Earnings Surprise Event: Automotive Gross Margin Expansion", "Tesla reported Q2 gross margin of 18.2% exceeding Wall Street consensus by 140 basis points.", "2025-07-24"),
        ("INTC", "Profit Warning & Restructuring Catalyst", "Intel announces foundry segment restructuring and delayed fab timeline, generating -6.8% abnormal negative return.", "2025-08-14"),
    ];

    base_events
        .into_iter()
        .map(|(ticker, title, body, date)| SearchableItem {
            id: format!("event_{}", Uuid::new_v4()),
            item_type: "event".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

fn collect_insider_items() -> Vec<SearchableItem> {
    let base_insider = [
        ("AAPL", "Tim Cook (CEO) Open Market Sale of 50,000 Shares of AAPL", "CEO Tim Cook executed a pre-planned Rule 10b5-1 trading plan sale of 50,000 common shares at average price of $232.50.", "2025-08-20"),
        ("NVDA", "Jensen Huang (CEO) Option Exercise & Share Disposition of NVDA", "President & CEO Jensen Huang reported option exercise and sale of 120,000 shares under Rule 10b5-1 plan at $128.40.", "2025-08-22"),
        ("MSFT", "Satya Nadella (CEO) Open Market Purchase of 15,000 Shares of MSFT", "Chairman & CEO Satya Nadella purchased 15,000 shares of Microsoft stock in open market transaction at $445.00.", "2025-08-18"),
        ("JPM", "Jamie Dimon (Chairman & CEO) Open Market Purchase of JPM Stock", "CEO Jamie Dimon acquired 25,000 shares of JPMorgan Chase & Co. common stock reflecting high executive conviction.", "2025-08-15"),
        ("TSLA", "Robyn Denholm (Chair of Board) Option Exercise of TSLA Shares", "Board Chair Robyn Denholm exercised non-qualified stock options for 20,000 TSLA shares at exercise price of $82.40.", "2025-08-10"),
    ];

    base_insider
        .into_iter()
        .map(|(ticker, title, body, date)| SearchableItem {
            id: format!("insider_{}", Uuid::new_v4()),
            item_type: "insider".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

fn collect_supply_chain_items() -> Vec<SearchableItem> {
    let default_sc = [
        ("TSM", "NVDA", "TSMC Foundry Fab Fabrication Dependency for NVIDIA Blackwell GPUs", "Taiwan Semiconductor Manufacturing Co. provides 100% advanced CoWoS packaging and 3nm node wafer fabrication for NVIDIA AI accelerators.", "2025-08-30"),
        ("ASML", "TSM", "ASML Extreme Ultraviolet (EUV) Lithography Tool Supply for TSMC", "ASML Holding N.V. supplies High-NA EUV lithography systems essential for TSMC leading-edge semiconductor nodes.", "2025-08-30"),
        ("AAPL", "FOXCONN", "Apple Device Assembly and Precision Manufacturing Dependency on Hon Hai", "Foxconn provides final assembly and testing for iPhone 16 Pro and Apple Vision Pro hardware devices.", "2025-08-30"),
        ("MU", "NVDA", "Micron High-Bandwidth Memory (HBM3e) Supply for NVIDIA AI Clusters", "Micron Technology supplies ultra-high bandwidth 24GB HBM3e memory stacks for NVIDIA H200 and B200 Tensor Core GPUs.", "2025-08-30"),
    ];

    default_sc
        .into_iter()
        .map(|(src, target, title, body, date)| SearchableItem {
            id: format!("sc_{}_{}", src, target),
            item_type: "supply_chain".to_string(),
            ticker: src.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

fn collect_options_items() -> Vec<SearchableItem> {
    let base_options = [
        ("AAPL", "Apple Unusual Call Option Activity: $250 Strike Exp 2025-12-19", "Unusual options activity detected on AAPL $250 Calls with volume 48,500 vs open interest 2,200 (Vol/OI ratio 22.0x, score 554.8).", "2025-08-29T12:00:00Z"),
        ("NVDA", "NVIDIA Unusual Call Option Flow: $140 Strike Exp 2025-12-19", "Aggressive institutional call buying on NVDA $140 strike with volume 95,000 contracts and implied volatility 58.4%.", "2025-08-29T12:00:00Z"),
        ("MSFT", "Microsoft Put/Call Ratio Shift: Elevated Put Skew (PCR 1.45)", "Market-wide put/call ratio on MSFT rose to 1.45 indicating short-term institutional portfolio hedging.", "2025-08-29T12:00:00Z"),
        ("TSLA", "Tesla Volatility Surface Smile: 78% ATM Implied Volatility", "2D options volatility surface skew shows elevated demand for out-of-the-money upside calls across next 3 monthly expirations.", "2025-08-29T12:00:00Z"),
        ("AMZN", "Amazon Unusual Options Activity: $200 Calls Exp 2025-10-17", "High volume unusual flow on AMZN $200 Call options with 32,000 contracts traded against 4,100 open interest.", "2025-08-29T12:00:00Z"),
    ];

    base_options
        .into_iter()
        .map(|(ticker, title, body, date)| SearchableItem {
            id: format!("options_{}", Uuid::new_v4()),
            item_type: "options".to_string(),
            ticker: ticker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            date: date.to_string(),
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Unified Search Handler
// ─────────────────────────────────────────────────────────────────────────────

/// Unified Cross-Domain Search Endpoint (`GET /search`).
///
/// Searches across News Articles, Earnings Call Transcripts, SEC Filings,
/// Sentiment Records, Corporate Events, Insider Trades, Supply Chain Links,
/// and Options Flow with relevance scoring, date filtering, and pagination.
#[utoipa::path(
    get,
    path = "/search",
    tag = "Search",
    params(
        SearchParams
    ),
    responses(
        (status = 200, description = "Aggregated and ranked search results", body = SearchResponse),
        (status = 400, description = "Bad request (empty query or invalid parameters)", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::auth::AuthErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn get_search_handler(
    State(state): State<AppState>,
    claims: Extension<Claims>,
    Query(params): Query<SearchParams>,
) -> Response {
    let query_str = params.q.trim();
    if query_str.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "Search query 'q' cannot be empty."
            })),
        )
            .into_response();
    }

    let limit = params.limit.unwrap_or(20);
    if limit == 0 || limit > 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": "limit must be between 1 and 100."
            })),
        )
            .into_response();
    }

    let offset = params.offset.unwrap_or(0);

    // Parse requested data domains
    let raw_types = params.types.as_deref().unwrap_or("all").trim();
    let selected_types: Vec<String> =
        if raw_types.eq_ignore_ascii_case("all") || raw_types.is_empty() {
            VALID_SEARCH_TYPES.iter().map(|s| s.to_string()).collect()
        } else {
            let mut parsed = Vec::new();
            for t in raw_types.split(',') {
                let item_t = t.trim().to_lowercase();
                if item_t.is_empty() {
                    continue;
                }
                if !VALID_SEARCH_TYPES.contains(&item_t.as_str()) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "Bad Request",
                            "message": format!(
                                "Invalid data type '{}'. Allowed types: {}",
                                item_t,
                                VALID_SEARCH_TYPES.join(", ")
                            )
                        })),
                    )
                        .into_response();
                }
                if !parsed.contains(&item_t) {
                    parsed.push(item_t);
                }
            }
            if parsed.is_empty() {
                VALID_SEARCH_TYPES.iter().map(|s| s.to_string()).collect()
            } else {
                parsed
            }
        };

    // Gather candidate items from selected domains
    let mut candidates: Vec<SearchableItem> = Vec::new();

    for domain in &selected_types {
        match domain.as_str() {
            "news" => candidates.extend(collect_news_items(&state)),
            "transcripts" => candidates.extend(collect_transcript_items(&state, &claims.sub)),
            "sentiment" => candidates.extend(collect_sentiment_items()),
            "filings" => candidates.extend(collect_filings_items()),
            "events" => candidates.extend(collect_events_items()),
            "insider" => candidates.extend(collect_insider_items()),
            "supply_chain" => candidates.extend(collect_supply_chain_items()),
            "options" => candidates.extend(collect_options_items()),
            _ => {}
        }
    }

    // Score and filter candidates
    let mut scored_results: Vec<SearchResultItem> = Vec::new();

    for item in candidates {
        // Date filtering
        if !matches_date_filter(
            &item.date,
            params.start_date.as_deref(),
            params.end_date.as_deref(),
        ) {
            continue;
        }

        let score = compute_relevance_score(query_str, &item.ticker, &item.title, &item.body);
        if score > 0.0 {
            scored_results.push(SearchResultItem {
                id: item.id,
                item_type: item.item_type,
                ticker: item.ticker,
                title: item.title,
                snippet: truncate_snippet(&item.body, 200),
                date: item.date,
                score,
            });
        }
    }

    // Sort by score DESC, then date DESC
    scored_results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.date.cmp(&a.date))
    });

    // Apply pagination
    let paginated_results: Vec<SearchResultItem> = scored_results
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    let count = paginated_results.len();

    let response = SearchResponse {
        query: query_str.to_string(),
        types: selected_types,
        count,
        results: paginated_results,
        generated_at: Utc::now().to_rfc3339(),
    };

    (StatusCode::OK, Json(response)).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_relevance_score_exact_ticker() {
        let score = compute_relevance_score("AAPL", "AAPL", "Some title", "Some body text");
        assert_eq!(score, 1.0);

        let score_case = compute_relevance_score("aapl", "AAPL", "Some title", "Some body text");
        assert_eq!(score_case, 1.0);
    }

    #[test]
    fn test_compute_relevance_score_title_and_prefix() {
        let score = compute_relevance_score(
            "quarterly",
            "AAPL",
            "Quarterly earnings report",
            "Some body text",
        );
        assert!(score >= 0.80 && score <= 1.0);

        let score_mid = compute_relevance_score(
            "earnings",
            "AAPL",
            "Apple announces record earnings",
            "Body text",
        );
        assert!(score_mid >= 0.80);
    }

    #[test]
    fn test_compute_relevance_score_body_match() {
        let score = compute_relevance_score(
            "blackwell",
            "NVDA",
            "Hardware Announcement",
            "NVIDIA revealed Blackwell Ultra chips.",
        );
        assert_eq!(score, 0.60);
    }

    #[test]
    fn test_compute_relevance_score_no_match() {
        let score = compute_relevance_score(
            "NONEXISTENT_KEYWORD",
            "AAPL",
            "Apple News",
            "Normal content",
        );
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_matches_date_filter() {
        assert!(matches_date_filter(
            "2025-08-15T10:00:00Z",
            Some("2025-08-01"),
            Some("2025-08-31")
        ));
        assert!(!matches_date_filter(
            "2025-07-15T10:00:00Z",
            Some("2025-08-01"),
            Some("2025-08-31")
        ));
        assert!(!matches_date_filter(
            "2025-09-05T10:00:00Z",
            Some("2025-08-01"),
            Some("2025-08-31")
        ));
        assert!(matches_date_filter("2025-08-15", None, None));
    }

    #[test]
    fn test_truncate_snippet() {
        let short = "Hello World";
        assert_eq!(truncate_snippet(short, 20), "Hello World");

        let long = "A".repeat(300);
        let truncated = truncate_snippet(&long, 200);
        assert_eq!(truncated.chars().count(), 203); // 200 chars + "..."
        assert!(truncated.ends_with("..."));
    }
}
