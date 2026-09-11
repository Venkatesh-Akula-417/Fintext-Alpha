use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// Query parameters for the Event Study / CAR endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct EventStudyParams {
    /// Underlying stock ticker symbol (e.g. "AAPL", "NVDA").
    pub ticker: String,
    /// Date of the corporate event in ISO YYYY-MM-DD format (e.g. "2025-05-15").
    pub event_date: String,
    /// Number of trading days before and after event date (1..=20, default: 5).
    pub event_window: Option<u32>,
    /// Number of trading days before event window for OLS estimation (10..=120, default: 60).
    pub estimation_window: Option<u32>,
    /// Benchmark ticker symbol for market model regression (default: "SPY").
    pub benchmark_ticker: Option<String>,
}

/// A single day's abnormal return and cumulative abnormal return observation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct AbnormalReturnPoint {
    /// Trading date in ISO YYYY-MM-DD format.
    pub date: String,
    /// Trading day offset relative to event date (e.g., -5, -1, 0, +1, +5).
    pub day_offset: i32,
    /// Actual asset return on this date: (P_t - P_{t-1}) / P_{t-1}.
    pub actual_return: f64,
    /// Benchmark return on this date: (P_m,t - P_m,{t-1}) / P_m,{t-1}.
    pub benchmark_return: f64,
    /// Model expected return on this date: alpha + beta * r_benchmark.
    pub expected_return: f64,
    /// Abnormal return on this date: actual_return - expected_return.
    pub abnormal_return: f64,
    /// Cumulative abnormal return from start of event window through this date.
    pub cumulative_abnormal_return: f64,
}

/// Response payload containing event study statistics and CAR trajectory.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EventStudyResponse {
    /// Analyzed stock ticker symbol.
    pub ticker: String,
    /// Corporate event date (YYYY-MM-DD).
    pub event_date: String,
    /// Event window size in trading days before/after event.
    pub event_window: u32,
    /// Estimation window size in trading days.
    pub estimation_window: u32,
    /// Benchmark symbol used for market model.
    pub benchmark_ticker: String,
    /// OLS market model intercept (Alpha).
    pub alpha: f64,
    /// OLS market model slope / sensitivity (Beta).
    pub beta: f64,
    /// Coefficient of determination (R^2) of the market model fit over estimation window.
    pub r_squared: f64,
    /// Cumulative abnormal return across the entire event window [-W_e, +W_e].
    pub car_full_window: f64,
    /// Cumulative abnormal return for the pre-event window [-W_e, -1] (information leakage / run-up).
    pub car_pre_event: f64,
    /// Cumulative abnormal return for the post-event window [+1, +W_e] (post-announcement drift).
    pub car_post_event: f64,
    /// Number of observations in the event window.
    pub count: usize,
    /// Daily abnormal return series across the event window.
    pub abnormal_returns: Vec<AbnormalReturnPoint>,
    /// Summary description or methodology message.
    pub message: String,
}

/// Query parameters for the 8-K filings endpoint (`GET /events/8k`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct EightKParams {
    /// Optional target equity ticker symbol (e.g. "AAPL", "NVDA"). If omitted, scans all tracked tickers.
    pub ticker: Option<String>,
    /// Optional event type filter (e.g. "M&A", "CEO Change", "Earnings Warning", "Bankruptcy", "Regulatory Investigation").
    pub event_type: Option<String>,
    /// Lookback window in calendar days (1..=30, default: 7).
    pub days: Option<u32>,
    /// Maximum number of filings to return (1..=100, default: 20).
    pub limit: Option<usize>,
}

/// A single SEC Form 8-K regulatory filing item.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EightKFiling {
    /// Underlying stock ticker symbol (e.g., "AAPL").
    pub ticker: String,
    /// SEC filing date in ISO YYYY-MM-DD format (e.g., "2026-08-25").
    pub filing_date: String,
    /// Form type (always "8-K").
    pub form_type: String,
    /// Classified event category (e.g. "M&A", "CEO Change", "Earnings Warning", "Bankruptcy", "Regulatory Investigation").
    pub event_type: String,
    /// Summary description of the material event reported.
    pub description: String,
    /// List of 8-K item codes reported (e.g., ["Item 1.01", "Item 2.01"]).
    pub items: Vec<String>,
    /// Official SEC EDGAR accession number (e.g. "0000320193-26-000042").
    pub accession_number: String,
    /// Direct link to SEC EDGAR document archive URL.
    pub url: String,
}

/// Response payload containing recent 8-K regulatory filings and event classifications.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EightKResponse {
    /// Ticker filter applied or "ALL".
    pub ticker: String,
    /// Event type filter applied or "ALL".
    pub event_type: String,
    /// Lookback period in days applied.
    pub days: u32,
    /// Total number of matching filings returned.
    pub count: usize,
    /// Array of classified 8-K filing records.
    pub filings: Vec<EightKFiling>,
    /// Summary status message.
    pub message: String,
}

/// Query parameters for the Earnings Surprise Tracker endpoint (`GET /events/earnings-surprise`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct EarningsSurpriseParams {
    /// Optional target equity ticker symbol (e.g. "AAPL", "NVDA"). If omitted, scans all tracked tickers.
    pub ticker: Option<String>,
    /// Start date of observation window in ISO YYYY-MM-DD format (default: 90 days ago).
    pub start_date: Option<String>,
    /// End date of observation window in ISO YYYY-MM-DD format (default: today).
    pub end_date: Option<String>,
    /// Minimum absolute change in average sentiment between pre- and post-event windows (0.05..=0.50, default: 0.15).
    pub min_sentiment_shift: Option<f64>,
    /// Number of trading days before earnings date to compute baseline sentiment (1..=10, default: 5).
    pub pre_days: Option<u32>,
    /// Number of trading days on/after earnings date to compute reaction sentiment (1..=10, default: 5).
    pub post_days: Option<u32>,
    /// Maximum number of earnings surprise events to return (1..=100, default: 20).
    pub limit: Option<usize>,
}

/// A quantified earnings surprise event with sentiment displacement metrics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EarningsSurpriseItem {
    /// Underlying stock ticker symbol (e.g., "AAPL").
    pub ticker: String,
    /// SEC 8-K Item 2.02 earnings release date in ISO YYYY-MM-DD format.
    pub earnings_date: String,
    /// Average baseline sentiment in the pre-event window.
    pub pre_avg_sentiment: f64,
    /// Average reaction sentiment in the post-event window.
    pub post_avg_sentiment: f64,
    /// Sentiment displacement score: post_avg_sentiment - pre_avg_sentiment.
    pub surprise_score: f64,
    /// Direction classification: "positive" if surprise_score >= 0.0 else "negative".
    pub direction: String,
    /// Number of sentiment records analyzed in the pre-event window.
    pub pre_record_count: usize,
    /// Number of sentiment records analyzed in the post-event window.
    pub post_record_count: usize,
}

/// Response payload containing detected earnings surprises across the specified observation window.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct EarningsSurpriseResponse {
    /// Target ticker filter applied or None if universe-wide scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Start date of observation window (YYYY-MM-DD).
    pub start_date: String,
    /// End date of observation window (YYYY-MM-DD).
    pub end_date: String,
    /// Minimum absolute sentiment shift threshold applied.
    pub min_sentiment_shift: f64,
    /// Pre-event baseline window in trading days.
    pub pre_days: u32,
    /// Post-event reaction window in trading days.
    pub post_days: u32,
    /// Total number of surprise events returned.
    pub count: usize,
    /// Array of detected earnings surprise events sorted by absolute surprise score descending.
    pub surprises: Vec<EarningsSurpriseItem>,
    /// ISO 8601 UTC timestamp of calculation.
    pub generated_at: String,
}

/// Query parameters for the Insider Trading Signal endpoint (`GET /events/insider-trading`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct InsiderTradingParams {
    /// Optional target equity ticker symbol (e.g. "AAPL", "NVDA"). If omitted, scans across tracked market universe.
    pub ticker: Option<String>,
    /// Transaction type filter ("purchase", "sale", "grant", "exercise", "all", default: "all").
    pub transaction_type: Option<String>,
    /// Start date of observation window in ISO YYYY-MM-DD format (default: 90 days ago).
    pub start_date: Option<String>,
    /// End date of observation window in ISO YYYY-MM-DD format (default: today).
    pub end_date: Option<String>,
    /// Minimum number of shares traded filter (default: 0).
    pub min_shares: Option<u64>,
    /// Minimum absolute signal score filter (0.0..=1.0, default: 0.0).
    pub min_signal_score: Option<f64>,
    /// Maximum number of insider transactions to return (1..=100, default: 20).
    pub limit: Option<usize>,
}

/// An individual SEC Form 4 insider trading transaction with normalized signal strength metrics.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct InsiderTradeItem {
    /// Underlying stock ticker symbol (e.g., "AAPL").
    pub ticker: String,
    /// Full legal name of the reporting corporate insider.
    pub insider_name: String,
    /// Corporate insider role (e.g., "CEO", "CFO", "Director", "Officer", "10% Owner").
    pub insider_role: String,
    /// Classified transaction type ("purchase", "sale", "grant", "exercise").
    pub transaction_type: String,
    /// Number of shares involved in the transaction.
    pub shares: u64,
    /// Execution price per share in USD.
    pub price: f64,
    /// Total gross transaction value in USD (shares * price).
    pub value: f64,
    /// SEC Form 4 filing date in ISO YYYY-MM-DD format.
    pub filing_date: String,
    /// Normalized directional conviction signal score (-1.0 to +1.0).
    pub signal_score: f64,
    /// Regulatory data origin (always "SEC Form 4").
    pub source: String,
}

/// Response payload containing recent SEC Form 4 insider transactions and quantified signal scores.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct InsiderTradingResponse {
    /// Target ticker filter applied or None if universe-wide scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Transaction type filter applied ("all", "purchase", "sale", "grant", "exercise").
    pub transaction_type: String,
    /// Start date of observation window (YYYY-MM-DD).
    pub start_date: String,
    /// End date of observation window (YYYY-MM-DD).
    pub end_date: String,
    /// Minimum shares threshold applied.
    pub min_shares: u64,
    /// Minimum signal score threshold applied.
    pub min_signal_score: f64,
    /// Total number of matching insider transactions returned.
    pub count: usize,
    /// Array of insider trading transactions sorted by filing date descending.
    pub trades: Vec<InsiderTradeItem>,
    /// ISO 8601 UTC timestamp of calculation.
    pub generated_at: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// M&A Rumor Detection Models
// ═══════════════════════════════════════════════════════════════════════════════

/// Query parameters for the M&A Rumor Detection endpoint (`GET /events/ma-rumors`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct MARumorsParams {
    /// Optional target equity ticker symbol (e.g. "AAPL", "NVDA"). If omitted, scans across tracked market universe.
    pub ticker: Option<String>,
    /// Minimum composite rumor conviction score threshold (0.0..=1.0, default: 0.5).
    pub min_rumor_score: Option<f64>,
    /// Lookback window in calendar days (1..=30, default: 7).
    pub lookback_days: Option<u32>,
    /// Maximum number of flagged companies to return (1..=100, default: 20).
    pub limit: Option<usize>,
}

/// A flagged company with quantified M&A rumor metrics and component breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct MARumorItem {
    /// Underlying stock ticker symbol (e.g., "AAPL").
    pub ticker: String,
    /// Composite M&A rumor conviction score (0.0 to 1.0).
    pub rumor_score: f64,
    /// Recent sentiment anomaly z-score deviation.
    pub sentiment_zscore: f64,
    /// Frequency count of M&A keywords detected in recent news and transcripts.
    pub keyword_hits: usize,
    /// Whether recent SEC Form 8-K filings contain M&A material event items (Item 1.01 or 2.01).
    pub recent_8k_ma_flag: bool,
    /// Related supplier and customer tickers from the institutional supply chain knowledge graph.
    pub supply_chain_related_tickers: Vec<String>,
    /// Whether recent SEC Form 4 filings exhibit net corporate insider accumulation.
    pub insider_net_buying: bool,
    /// Headline title of the most recent significant news article or filing.
    pub latest_news_title: String,
    /// ISO 8601 UTC timestamp of detection.
    pub generated_at: String,
}

/// Response payload for the M&A Rumor Detection endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct MARumorsResponse {
    /// Target ticker filter applied or None if universe-wide scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Applied minimum rumor score threshold.
    pub min_rumor_score: f64,
    /// Lookback window in calendar days.
    pub lookback_days: u32,
    /// Total number of flagged M&A rumor candidates returned.
    pub count: usize,
    /// Array of flagged M&A rumor items sorted by composite score descending.
    pub items: Vec<MARumorItem>,
    /// ISO 8601 UTC timestamp of calculation.
    pub generated_at: String,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Regulatory Filings Classifier Models
// ═══════════════════════════════════════════════════════════════════════════════

/// Query parameters for the Regulatory Filings endpoint (`GET /events/filings`).
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct RegulatoryFilingsParams {
    /// Optional target equity ticker symbol (e.g. "AAPL", "NVDA"). If omitted, scans across tracked universe.
    pub ticker: Option<String>,
    /// Optional SEC form type filter (e.g. "10-K", "10-Q", "8-K", "S-1", "DEF 14A", "Form 4").
    pub form_type: Option<String>,
    /// Optional classified event category filter (e.g. "Annual Report", "Quarterly Report", "M&A", "Bankruptcy", "Earnings", "Governance", "Registration Statement", "Proxy Statement", "Insider Transaction", "Foreign Issuer Report", "Other").
    pub event_category: Option<String>,
    /// Earliest filing date in ISO YYYY-MM-DD format (default: 90 days ago).
    pub start_date: Option<String>,
    /// Latest filing date in ISO YYYY-MM-DD format (default: today).
    pub end_date: Option<String>,
    /// Maximum number of filings to return (1..=100, default: 20).
    pub limit: Option<usize>,
    /// Number of records to skip for pagination (default: 0).
    pub offset: Option<usize>,
}

/// A single SEC regulatory filing with classified event category and EDGAR metadata.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RegulatoryFilingItem {
    /// Underlying stock ticker symbol (e.g., "AAPL").
    pub ticker: String,
    /// Official SEC form type (e.g., "10-K", "10-Q", "8-K", "S-1", "DEF 14A", "Form 4").
    pub form_type: String,
    /// SEC filing acceptance date in ISO YYYY-MM-DD format.
    pub filing_date: String,
    /// Official SEC EDGAR accession number (e.g., "0000320193-25-000010").
    pub accession_number: String,
    /// Standardized event category classified from form type and disclosure content.
    pub event_category: String,
    /// Descriptive title or executive summary of the filing.
    pub description: String,
    /// Official SEC EDGAR filing document URL.
    pub url: String,
}

/// Response payload containing classified SEC regulatory filings and pagination metadata.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct RegulatoryFilingsResponse {
    /// Target ticker filter applied or None if universe-wide scan.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Form type filter applied or None if all forms included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_type: Option<String>,
    /// Event category filter applied or None if all categories included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_category: Option<String>,
    /// Observation start date (YYYY-MM-DD).
    pub start_date: String,
    /// Observation end date (YYYY-MM-DD).
    pub end_date: String,
    /// Number of filings returned in this page.
    pub count: usize,
    /// Total number of matching filings across all pages.
    pub total_count: usize,
    /// Pagination offset applied.
    pub offset: usize,
    /// Pagination limit applied.
    pub limit: usize,
    /// Array of classified regulatory filings.
    pub filings: Vec<RegulatoryFilingItem>,
    /// ISO 8601 UTC timestamp of calculation.
    pub generated_at: String,
}
