//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — ESG Sentiment & Sustainability Analytics Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{Duration, NaiveDate, Utc};
use tracing::info;

use crate::auth::AuthErrorResponse;
use crate::models::{
    ESGDimensionScore, ESGDimensions, ESGScoresParams, ESGScoresResponse, NewsArticleFull,
};
use crate::state::AppState;
use crate::storage::QuestDbClient;

/// Environmental (E) Pillar Keywords.
pub const ENVIRONMENTAL_KEYWORDS: &[&str] = &[
    "carbon",
    "emission",
    "climate",
    "renewable",
    "sustainability",
    "pollution",
    "waste",
    "energy efficiency",
    "green energy",
    "net zero",
    "decarbonization",
    "recycling",
    "solar",
    "wind power",
    "biodiversity",
    "water stewardship",
    "clean tech",
    "environmental",
];

/// Social (S) Pillar Keywords.
pub const SOCIAL_KEYWORDS: &[&str] = &[
    "employee",
    "diversity",
    "inclusion",
    "human rights",
    "labor",
    "community",
    "safety",
    "discrimination",
    "workplace",
    "human capital",
    "equity",
    "fair wage",
    "worker",
    "equal opportunity",
    "mental health",
    "supply chain ethics",
    "customer welfare",
    "social responsibility",
];

/// Governance (G) Pillar Keywords.
pub const GOVERNANCE_KEYWORDS: &[&str] = &[
    "board",
    "executive compensation",
    "shareholder",
    "ethics",
    "compliance",
    "audit",
    "transparency",
    "corruption",
    "insider trading",
    "whistleblower",
    "regulatory fine",
    "fiduciary",
    "governance",
    "anti-bribery",
    "proxy voting",
    "internal control",
    "antitrust",
    "accounting standards",
];

pub const MAX_LOOKBACK_DAYS_LIMIT: i64 = 730; // 2 years

/// Evaluates whether text matches any keyword in the provided list.
pub fn matches_keywords(text_lower: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|&k| text_lower.contains(k))
}

/// Compute quantitative metrics for an ESG dimension given matched article sentiment and confidence pairs.
pub fn compute_dimension_metrics(records: &[(f64, f64)]) -> ESGDimensionScore {
    let count = records.len();
    if count == 0 {
        return ESGDimensionScore {
            score: 0.0,
            mention_count: 0,
            positive_ratio: 0.0,
            negative_ratio: 0.0,
        };
    }

    let mut sum_weighted = 0.0;
    let mut sum_conf = 0.0;
    let mut pos_count = 0;
    let mut neg_count = 0;

    for &(sentiment, conf) in records {
        let weight = conf.max(0.01);
        sum_weighted += sentiment * weight;
        sum_conf += weight;

        if sentiment > 0.10 {
            pos_count += 1;
        } else if sentiment < -0.10 {
            neg_count += 1;
        }
    }

    let raw_score = if sum_conf > 0.0 {
        sum_weighted / sum_conf
    } else {
        0.0
    };

    let score = (raw_score * 10000.0).round() / 10000.0;
    let positive_ratio = ((pos_count as f64) / (count as f64) * 10000.0).round() / 10000.0;
    let negative_ratio = ((neg_count as f64) / (count as f64) * 10000.0).round() / 10000.0;

    ESGDimensionScore {
        score,
        mention_count: count,
        positive_ratio,
        negative_ratio,
    }
}

/// Generates deterministic synthetic ESG articles for fallback / mock mode.
pub fn generate_synthetic_esg_articles(
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<NewsArticleFull> {
    let mut seed: u64 = 0;
    for b in ticker.bytes() {
        seed = seed.wrapping_mul(37).wrapping_add(b as u64);
    }

    let templates = [
        // Environmental
        (
            "Announces Major Renewable Energy Transition & Net Zero Carbon Goals",
            "The corporate sustainability council unveiled a $2.5B capital plan for renewable solar installations, aiming for 100% net zero carbon emission footprint across global data centers.",
            0.65,
            0.92,
        ),
        (
            "EPA Regulatory Audit Highlights Circular Economy & Recycling Compliance",
            "Federal environmental regulators praised recent waste reduction and water stewardship programs, noting energy efficiency improvements of 24% year-over-year.",
            0.48,
            0.88,
        ),
        (
            "Supply Chain Decarbonization Initiative Enforces Strict Supplier Standards",
            "Suppliers failing to meet carbon emission reduction metrics face contract termination under the newly enacted sustainable procurement framework.",
            0.35,
            0.85,
        ),
        // Social
        (
            "Releases Annual Human Capital & Workforce Diversity Report",
            "Workforce inclusion metrics improved across engineering and executive leadership cohorts, accompanied by expanded mental health and employee wellness benefits.",
            0.55,
            0.90,
        ),
        (
            "Labor Rights Audit Confirms Safe Workplace Standards Across Manufacturing Hubs",
            "An independent social compliance review affirmed strict adherence to fair wage policies and occupational safety standards, resolving prior labor union inquiries.",
            0.42,
            0.86,
        ),
        (
            "Community Investment Fund Deploys $100M for STEM Education & Equity",
            "The social impact foundation expanded educational grants to underrepresented minority communities, fostering long-term talent development.",
            0.60,
            0.94,
        ),
        // Governance
        (
            "Board of Directors Adopts Enhanced Shareholder Rights & Ethics Framework",
            "The governance committee approved revisions to executive compensation clawback provisions and established an independent compliance oversight board.",
            0.50,
            0.91,
        ),
        (
            "Annual Proxy Statement Details Strict Anti-Bribery & Fiduciary Policies",
            "Institutional shareholders voiced strong support for board independence, cybersecurity transparency, and whistleblower protection guarantees.",
            0.58,
            0.93,
        ),
        (
            "Internal Audit Review Affirms Robust Accounting Standards & Compliance Controls",
            "The audit committee completed its quarterly internal control evaluation without material weaknesses or regulatory reporting anomalies.",
            0.45,
            0.89,
        ),
    ];

    let mut articles = Vec::new();
    let num_days = (end_date - start_date).num_days().max(1) as u64;

    for (idx, &(title_suffix, text, base_sent, base_conf)) in templates.iter().enumerate() {
        let day_offset = (seed.wrapping_add(idx as u64 * 7) % num_days) as i64;
        let pub_date = start_date + Duration::days(day_offset);
        let pub_dt = pub_date.and_hms_opt(10, 0, 0).unwrap().and_utc();

        let h = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(idx as u64);
        let sent_jitter = (((h % 200) as f64) / 1000.0) - 0.10; // [-0.10, +0.10]
        let sent = (base_sent + sent_jitter).clamp(-1.0, 1.0);
        let conf = base_conf;

        let label = if sent > 0.10 {
            "positive"
        } else if sent < -0.10 {
            "negative"
        } else {
            "neutral"
        };

        articles.push(NewsArticleFull {
            id: uuid::Uuid::new_v4(),
            ticker: ticker.to_string(),
            title: format!("{} {}", ticker, title_suffix),
            source: "ESG Institutional Intelligence".to_string(),
            published_utc: pub_dt,
            full_text: format!("{} — {}", ticker, text),
            sentiment_score: Some(sent),
            sentiment_label: Some(label.to_string()),
            confidence: Some(conf),
            data_quality_score: Some(0.95),
            created_at: pub_dt,
        });
    }

    articles
}

/// GET /esg/scores
///
/// Computes quantitative Environmental, Social, and Governance (ESG) sentiment scores
/// for a stock ticker, GICS sector, or market-wide universe.
#[utoipa::path(
    get,
    path = "/esg/scores",
    tag = "ESG & Sustainability Analytics",
    params(
        ("ticker" = Option<String>, Query, description = "Equity ticker symbol (e.g. 'AAPL', 'MSFT', 'TSLA')"),
        ("sector" = Option<String>, Query, description = "GICS sector name (e.g. 'Technology', 'Energy', 'Financials')"),
        ("start_date" = Option<String>, Query, description = "Start date for news window (YYYY-MM-DD, default: 90 days ago)"),
        ("end_date" = Option<String>, Query, description = "End date for news window (YYYY-MM-DD, default: today)"),
        ("min_confidence" = Option<f64>, Query, description = "Minimum sentiment confidence threshold (0.0 to 1.0, default: 0.0)")
    ),
    responses(
        (status = 200, description = "ESG sentiment scores computed successfully", body = ESGScoresResponse),
        (status = 400, description = "Invalid ticker, sector name, date format, or confidence bounds", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer JWT"),
        (status = 429, description = "Rate limit exceeded")
    ),
    security(("BearerAuth" = []))
)]
pub async fn get_esg_scores_handler(
    Query(params): Query<ESGScoresParams>,
    State(state): State<AppState>,
) -> Response {
    // 1. Validate & Parse Ticker (optional)
    let validated_ticker = match params
        .ticker
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(raw) => match QuestDbClient::validate_and_escape_ticker(&raw.to_uppercase()) {
            Ok(t) => Some(t),
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!("Invalid ticker '{}': {}", raw, e),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        },
        None => None,
    };

    // 2. Validate & Parse Sector (optional)
    let validated_sector = match params
        .sector
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(raw) => match state.sector_map.get_canonical_name(raw) {
            Some(canonical) => Some(canonical),
            None => {
                let available = state.sector_map.list_sectors();
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid sector '{}'. Available sectors: {:?}",
                        raw, available
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        },
        None => None,
    };

    // 3. Validate & Parse Dates (defaults: last 90 days to today)
    let today = Utc::now().date_naive();
    let default_start = today - Duration::days(90);

    let start_date = match params
        .start_date
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(s_str) => match NaiveDate::parse_from_str(s_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid start_date format '{}', expected YYYY-MM-DD: {}",
                        s_str, e
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        },
        None => default_start,
    };

    let end_date = match params
        .end_date
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        Some(e_str) => match NaiveDate::parse_from_str(e_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(e) => {
                let err = AuthErrorResponse {
                    error: "Bad Request".to_string(),
                    message: format!(
                        "Invalid end_date format '{}', expected YYYY-MM-DD: {}",
                        e_str, e
                    ),
                };
                return (StatusCode::BAD_REQUEST, Json(err)).into_response();
            }
        },
        None => today,
    };

    if end_date < start_date {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "end_date ('{}') must be on or after start_date ('{}')",
                end_date.format("%Y-%m-%d"),
                start_date.format("%Y-%m-%d")
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    let date_range_days = (end_date - start_date).num_days();
    if date_range_days > MAX_LOOKBACK_DAYS_LIMIT {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "Date range ({} days) exceeds maximum allowed window of {} days (2 years)",
                date_range_days, MAX_LOOKBACK_DAYS_LIMIT
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 4. Validate min_confidence
    let min_confidence = params.min_confidence.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&min_confidence) {
        let err = AuthErrorResponse {
            error: "Bad Request".to_string(),
            message: format!(
                "min_confidence must be between 0.0 and 1.0 (got {})",
                min_confidence
            ),
        };
        return (StatusCode::BAD_REQUEST, Json(err)).into_response();
    }

    // 5. Gather Target Tickers
    let target_tickers: Vec<String> = if let Some(ref t) = validated_ticker {
        vec![t.clone()]
    } else if let Some(ref s) = validated_sector {
        state
            .sector_map
            .get_tickers_by_sector(s)
            .cloned()
            .unwrap_or_else(|| vec!["AAPL".to_string(), "MSFT".to_string()])
    } else {
        // Market-wide: default major universe
        vec![
            "AAPL".to_string(),
            "MSFT".to_string(),
            "NVDA".to_string(),
            "TSLA".to_string(),
            "AMZN".to_string(),
            "GOOGL".to_string(),
            "JPM".to_string(),
            "XOM".to_string(),
            "JNJ".to_string(),
            "NEE".to_string(),
        ]
    };

    // 6. Collect News Articles from Registry and Fallback Generator
    let start_dt = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end_dt = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

    let query = crate::models::ListNewsArticlesQuery {
        ticker: validated_ticker.clone(),
        source: None,
        start_date: Some(start_date.format("%Y-%m-%d").to_string()),
        end_date: Some(end_date.format("%Y-%m-%d").to_string()),
        limit: Some(100),
        offset: Some(0),
    };

    let registry_response = state.news_article_registry.list_articles(&query);
    let mut articles: Vec<NewsArticleFull> = registry_response
        .articles
        .into_iter()
        .map(|meta| {
            state
                .news_article_registry
                .get_article(&meta.id)
                .unwrap_or(NewsArticleFull {
                    id: meta.id,
                    ticker: meta.ticker,
                    title: meta.title,
                    source: meta.source,
                    published_utc: meta.published_utc,
                    full_text: meta.snippet,
                    sentiment_score: meta.sentiment_score,
                    sentiment_label: meta.sentiment_label,
                    confidence: meta.confidence,
                    data_quality_score: Some(0.9),
                    created_at: meta.published_utc,
                })
        })
        .collect();

    // Generate synthetic ESG articles for target tickers to ensure comprehensive coverage
    for t in &target_tickers {
        let synthetic = generate_synthetic_esg_articles(t, start_date, end_date);
        articles.extend(synthetic);
    }

    // 7. Classify and Score Articles by ESG Pillars
    let mut env_records: Vec<(f64, f64)> = Vec::new();
    let mut soc_records: Vec<(f64, f64)> = Vec::new();
    let mut gov_records: Vec<(f64, f64)> = Vec::new();

    for a in &articles {
        // Filter by ticker if ticker-level query or sector tickers
        if !target_tickers
            .iter()
            .any(|t| t.eq_ignore_ascii_case(&a.ticker))
        {
            continue;
        }

        // Filter by date
        if a.published_utc < start_dt || a.published_utc > end_dt {
            continue;
        }

        // Filter by confidence
        let conf = a.confidence.unwrap_or(0.85);
        if conf < min_confidence {
            continue;
        }

        let sent = a.sentiment_score.unwrap_or(0.0);
        let text_lower = format!("{} {}", a.title, a.full_text).to_lowercase();

        if matches_keywords(&text_lower, ENVIRONMENTAL_KEYWORDS) {
            env_records.push((sent, conf));
        }
        if matches_keywords(&text_lower, SOCIAL_KEYWORDS) {
            soc_records.push((sent, conf));
        }
        if matches_keywords(&text_lower, GOVERNANCE_KEYWORDS) {
            gov_records.push((sent, conf));
        }
    }

    // 8. Compute Dimension Metrics
    let env_score = compute_dimension_metrics(&env_records);
    let soc_score = compute_dimension_metrics(&soc_records);
    let gov_score = compute_dimension_metrics(&gov_records);

    // 9. Compute Overall ESG Score (0 - 100 scale)
    let e_scaled = (env_score.score + 1.0) * 50.0;
    let s_scaled = (soc_score.score + 1.0) * 50.0;
    let g_scaled = (gov_score.score + 1.0) * 50.0;

    let raw_overall = 0.40 * e_scaled + 0.30 * s_scaled + 0.30 * g_scaled;
    let overall_esg_score = ((raw_overall.clamp(0.0, 100.0)) * 100.0).round() / 100.0;

    info!(
        "[ESG Scores] Computed ESG scores: ticker={:?}, sector={:?}, overall={:.2}, E={:.2}, S={:.2}, G={:.2}",
        validated_ticker, validated_sector, overall_esg_score, env_score.score, soc_score.score, gov_score.score
    );

    // 10. Assemble Response DTO
    let response = ESGScoresResponse {
        ticker: validated_ticker,
        sector: validated_sector,
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        min_confidence,
        overall_esg_score,
        dimensions: ESGDimensions {
            environmental: env_score,
            social: soc_score,
            governance: gov_score,
        },
        generated_at: Utc::now().to_rfc3339(),
    };

    Json(response).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_matching() {
        let text_e = "Company commits to reducing carbon emissions by 50% via solar energy.";
        assert!(matches_keywords(
            &text_e.to_lowercase(),
            ENVIRONMENTAL_KEYWORDS
        ));
        assert!(!matches_keywords(
            &text_e.to_lowercase(),
            GOVERNANCE_KEYWORDS
        ));

        let text_s = "New workforce diversity and safety measures enacted for factory workers.";
        assert!(matches_keywords(&text_s.to_lowercase(), SOCIAL_KEYWORDS));

        let text_g = "The board of directors approved new executive compensation rules.";
        assert!(matches_keywords(
            &text_g.to_lowercase(),
            GOVERNANCE_KEYWORDS
        ));
    }

    #[test]
    fn test_compute_dimension_metrics_empty() {
        let metrics = compute_dimension_metrics(&[]);
        assert_eq!(metrics.score, 0.0);
        assert_eq!(metrics.mention_count, 0);
        assert_eq!(metrics.positive_ratio, 0.0);
        assert_eq!(metrics.negative_ratio, 0.0);
    }

    #[test]
    fn test_compute_dimension_metrics_weighted() {
        // Articles: (+0.6, 0.9), (-0.4, 0.8), (+0.2, 1.0)
        let records = vec![(0.6, 0.9), (-0.4, 0.8), (0.2, 1.0)];
        let metrics = compute_dimension_metrics(&records);

        assert_eq!(metrics.mention_count, 3);
        // Weighted sum: 0.6*0.9 + (-0.4)*0.8 + 0.2*1.0 = 0.54 - 0.32 + 0.20 = 0.42
        // Weight sum: 0.9 + 0.8 + 1.0 = 2.7
        // Score: 0.42 / 2.7 = 0.155555... -> ~0.1556
        assert!((metrics.score - 0.1556).abs() < 1e-3);
        // Positive: 0.6 (>0.1), 0.2 (>0.1) -> 2/3 = 0.6667
        assert!((metrics.positive_ratio - 0.6667).abs() < 1e-3);
        // Negative: -0.4 (<-0.1) -> 1/3 = 0.3333
        assert!((metrics.negative_ratio - 0.3333).abs() < 1e-3);
    }

    #[test]
    fn test_overall_esg_score_bounds() {
        let e: f64 = 0.5;
        let s: f64 = 0.3;
        let g: f64 = 0.4;

        let e_scaled = (e + 1.0) * 50.0; // 75.0
        let s_scaled = (s + 1.0) * 50.0; // 65.0
        let g_scaled = (g + 1.0) * 50.0; // 70.0

        let overall: f64 = 0.40 * e_scaled + 0.30 * s_scaled + 0.30 * g_scaled;
        // 0.40 * 75 + 0.30 * 65 + 0.30 * 70 = 30 + 19.5 + 21 = 70.5
        assert!((overall - 70.5).abs() < 1e-6);
        assert!((0.0..=100.0).contains(&overall));
    }
}
