//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — News Articles Full Text Storage & Registry
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use dashmap::DashMap;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use crate::models::{
    ListNewsArticlesQuery, NewsArticleFull, NewsArticleMetadata, NewsArticlesListResponse,
};

/// In-memory and PostgreSQL synchronized News Article full-text storage registry.
#[derive(Debug, Clone, Default)]
pub struct NewsArticleRegistry {
    entries: Arc<DashMap<Uuid, NewsArticleFull>>,
}

impl NewsArticleRegistry {
    /// Creates a new empty `NewsArticleRegistry`.
    pub fn new() -> Self {
        let registry = Self {
            entries: Arc::new(DashMap::new()),
        };
        registry.seed_mock_news();
        registry
    }

    /// Initializes PostgreSQL schema for `news_articles`.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS news_articles (
                id UUID PRIMARY KEY,
                ticker TEXT NOT NULL,
                title TEXT NOT NULL,
                source TEXT NOT NULL,
                published_utc TIMESTAMPTZ NOT NULL,
                full_text TEXT NOT NULL,
                sentiment_score FLOAT8,
                sentiment_label TEXT,
                confidence FLOAT8,
                data_quality_score FLOAT8,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_news_articles_ticker_date ON news_articles(ticker, published_utc DESC);
        "#;
        sqlx::query(sql).execute(pool).await?;
        info!("[News Articles] PostgreSQL 'news_articles' table verified and indexed");
        Ok(())
    }

    /// Loads existing news articles from PostgreSQL into in-memory cache.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let sql = r#"
            SELECT id, ticker, title, source, published_utc, full_text,
                   sentiment_score, sentiment_label, confidence, data_quality_score, created_at
            FROM news_articles
            ORDER BY published_utc DESC
            LIMIT 5000
        "#;
        let rows = sqlx::query(sql).fetch_all(pool).await?;
        let count = rows.len();

        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let ticker: String = row.try_get("ticker")?;
            let title: String = row.try_get("title")?;
            let source: String = row.try_get("source")?;
            let published_utc: DateTime<Utc> = row.try_get("published_utc")?;
            let full_text: String = row.try_get("full_text")?;
            let sentiment_score: Option<f64> = row.try_get("sentiment_score")?;
            let sentiment_label: Option<String> = row.try_get("sentiment_label")?;
            let confidence: Option<f64> = row.try_get("confidence")?;
            let data_quality_score: Option<f64> = row.try_get("data_quality_score")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;

            self.entries.insert(
                id,
                NewsArticleFull {
                    id,
                    ticker,
                    title,
                    source,
                    published_utc,
                    full_text,
                    sentiment_score,
                    sentiment_label,
                    confidence,
                    data_quality_score,
                    created_at,
                },
            );
        }

        info!(
            "[News Articles] Loaded {} articles from PostgreSQL into memory cache",
            count
        );
        Ok(count)
    }

    /// Populates realistic institutional financial news for development and fallback mode.
    pub fn seed_mock_news(&self) {
        let now = Utc::now();

        let samples = vec![
            (
                "AAPL",
                "Apple Unveils Next-Gen Neural Engine & Enterprise Services Growth Outlook",
                "Institutional Wire",
                now - chrono::Duration::hours(2),
                "Apple Inc. announced a significant expansion of its enterprise cloud compute partnerships and next-generation neural processing capabilities during its institutional investor briefing in Cupertino. Senior leadership emphasized double-digit growth in high-margin services revenue and accelerated enterprise hardware refresh cycles. Operating margins for the services division expanded by 140 basis points year-over-year, driven by enterprise subscriptions and custom institutional AI workflow tooling. Supply chain partners in East Asia reported stable component yields and on-schedule mass production schedules for upcoming flagship devices.",
                Some(0.84),
                Some("positive".to_string()),
                Some(0.95),
                Some(0.98),
            ),
            (
                "NVDA",
                "NVIDIA Secures Multi-Billion Tier-1 Hyperscaler Cluster Commitments",
                "Bloomberg Terminal Feed",
                now - chrono::Duration::hours(5),
                "NVIDIA Corporation has finalized multi-year supply agreements with leading global hyperscalers for its advanced Blackwell architecture GPU clusters. The deal includes comprehensive networking interconnects powered by Quantum-X800 InfiniBand switches, expanding NVIDIA's enterprise data center TAM significantly. Financial analysts noted that software monetization through NVIDIA AI Enterprise licenses is growing at an annualized rate exceeding 45%, providing high-visibility recurring revenue streams that mitigate hardware cyclicality concerns.",
                Some(0.92),
                Some("positive".to_string()),
                Some(0.98),
                Some(0.99),
            ),
            (
                "MSFT",
                "Microsoft Azure Reports Record Commercial Cloud Revenue in Q3 Update",
                "SEC EDGAR",
                now - chrono::Duration::hours(9),
                "Microsoft Corporation filed an updated institutional 8-K disclosure detailing commercial cloud momentum, with Azure revenue accelerating to 31% constant-currency growth. Intelligent Cloud gross margins remained robust at 72%, benefiting from ongoing server infrastructure optimization and proprietary silicon deployments. Enterprise customer retention in Office 365 Copilot reached 94% among Fortune 500 pilot cohorts, reinforcing management guidance for fiscal full-year operating leverage.",
                Some(0.79),
                Some("positive".to_string()),
                Some(0.91),
                Some(0.96),
            ),
            (
                "TSLA",
                "Tesla Receives Regulatory Approval for Next-Generation Superfactory Expansion",
                "Dow Jones Newswires",
                now - chrono::Duration::hours(14),
                "Tesla Inc. secured comprehensive environmental and zoning clearances for its automated Gigafactory expansion project, enabling immediate deployment of next-generation unboxed assembly architecture. The facility is engineered to lower per-unit capital expenditures by 38% while doubling annual vehicle throughput for commercial fleets. Energy storage deployments also hit quarterly records with Megapack installations up 125% year-over-year.",
                Some(0.68),
                Some("positive".to_string()),
                Some(0.87),
                Some(0.93),
            ),
            (
                "AMZN",
                "Amazon AWS Announces Sovereign Cloud Infrastructure Across European Union",
                "Institutional Wire",
                now - chrono::Duration::hours(20),
                "Amazon.com Inc.'s cloud division AWS announced the operational launch of its dedicated Sovereign Cloud infrastructure in Europe, complying with stringent EU data residency and financial sector operational resilience mandates (DORA). Enterprise financial institutions across Frankfurt, Paris, and Amsterdam have begun migrating mission-critical risk computation pipelines to the new sovereign regions.",
                Some(0.72),
                Some("positive".to_string()),
                Some(0.89),
                Some(0.97),
            ),
            (
                "GOOGL",
                "Alphabet Outlines Quantum Supremacy Benchmark in Real-Time Portfolio Optimization",
                "Finnhub",
                now - chrono::Duration::hours(26),
                "Alphabet Inc. published breakthrough benchmark results demonstrating quantum-assisted algorithmic portfolio rebalancing across 10,000 global financial assets with sub-millisecond convergence times. The research, conducted in partnership with leading quantitative asset managers, highlights significant computational advantages over classical quadratic programming solvers.",
                Some(0.81),
                Some("positive".to_string()),
                Some(0.93),
                Some(0.95),
            ),
            (
                "AAPL",
                "Antitrust Regulatory Review Initiated for Digital App Store Fee Structures",
                "Institutional Wire",
                now - chrono::Duration::hours(36),
                "Regulatory authorities in the UK and European Commission have formally issued preliminary inquiries regarding in-app billing commission structures. Legal analysts anticipate extended compliance dialogues, though equity research teams project minimal structural impact on Apple's aggregate services gross profitability given geographic revenue diversification.",
                Some(-0.45),
                Some("negative".to_string()),
                Some(0.82),
                Some(0.94),
            ),
            (
                "NVDA",
                "Global Semiconductor Foundry Supply Lead Times Normalize Ahead of Schedule",
                "Finnhub",
                now - chrono::Duration::hours(48),
                "Advanced packaging and High-Bandwidth Memory (HBM3e) supply chains have achieved optimal throughput yields, reducing lead times for enterprise accelerator shipments to under six weeks. Industry channel checks confirm that component availability will support accelerated deliveries for Q4 institutional data center buildouts.",
                Some(0.65),
                Some("positive".to_string()),
                Some(0.86),
                Some(0.92),
            ),
        ];

        for (ticker, title, source, published, text, sentiment, label, conf, quality) in samples {
            let id = Uuid::new_v4();
            self.entries.insert(
                id,
                NewsArticleFull {
                    id,
                    ticker: ticker.to_string(),
                    title: title.to_string(),
                    source: source.to_string(),
                    published_utc: published,
                    full_text: text.to_string(),
                    sentiment_score: sentiment,
                    sentiment_label: label,
                    confidence: conf,
                    data_quality_score: quality,
                    created_at: published,
                },
            );
        }

        info!(
            "[News Articles] Seeded {} mock institutional financial news articles",
            self.entries.len()
        );
    }

    /// Lists news articles matching optional filters with pagination and snippet extraction.
    pub fn list_articles(&self, query: &ListNewsArticlesQuery) -> NewsArticlesListResponse {
        let ticker_filter = query.ticker.as_ref().map(|t| t.trim().to_uppercase());
        let source_filter = query.source.as_ref().map(|s| s.trim().to_lowercase());

        let start_date = query
            .start_date
            .as_ref()
            .and_then(|d| parse_date_filter(d, true));
        let end_date = query
            .end_date
            .as_ref()
            .and_then(|d| parse_date_filter(d, false));

        let limit = query.limit.unwrap_or(20).min(100).max(1);
        let offset = query.offset.unwrap_or(0);

        let mut matched: Vec<NewsArticleFull> = self
            .entries
            .iter()
            .map(|r| r.value().clone())
            .filter(|a| {
                if let Some(ref t) = ticker_filter {
                    if !a.ticker.eq_ignore_ascii_case(t) {
                        return false;
                    }
                }
                if let Some(ref s) = source_filter {
                    if !a.source.to_lowercase().contains(s) {
                        return false;
                    }
                }
                if let Some(start) = start_date {
                    if a.published_utc < start {
                        return false;
                    }
                }
                if let Some(end) = end_date {
                    if a.published_utc > end {
                        return false;
                    }
                }
                true
            })
            .collect();

        // Sort descending by published_utc
        matched.sort_by(|a, b| b.published_utc.cmp(&a.published_utc));

        let total = matched.len();
        let paginated: Vec<NewsArticleMetadata> = matched
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|a| a.to_metadata())
            .collect();

        NewsArticlesListResponse {
            articles: paginated,
            total,
            limit,
            offset,
        }
    }

    /// Retrieves a single full news article by UUID.
    pub fn get_article(&self, id: &Uuid) -> Option<NewsArticleFull> {
        self.entries.get(id).map(|r| r.value().clone())
    }

    /// Inserts or updates an article in memory and optionally persists to PostgreSQL.
    pub async fn save_article(
        &self,
        pool: Option<&PgPool>,
        article: NewsArticleFull,
    ) -> Result<(), sqlx::Error> {
        self.entries.insert(article.id, article.clone());

        if let Some(p) = pool {
            let sql = r#"
                INSERT INTO news_articles (
                    id, ticker, title, source, published_utc, full_text,
                    sentiment_score, sentiment_label, confidence, data_quality_score, created_at
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (id) DO UPDATE SET
                    ticker = EXCLUDED.ticker,
                    title = EXCLUDED.title,
                    source = EXCLUDED.source,
                    published_utc = EXCLUDED.published_utc,
                    full_text = EXCLUDED.full_text,
                    sentiment_score = EXCLUDED.sentiment_score,
                    sentiment_label = EXCLUDED.sentiment_label,
                    confidence = EXCLUDED.confidence,
                    data_quality_score = EXCLUDED.data_quality_score
            "#;
            sqlx::query(sql)
                .bind(article.id)
                .bind(&article.ticker)
                .bind(&article.title)
                .bind(&article.source)
                .bind(article.published_utc)
                .bind(&article.full_text)
                .bind(article.sentiment_score)
                .bind(&article.sentiment_label)
                .bind(article.confidence)
                .bind(article.data_quality_score)
                .bind(article.created_at)
                .execute(p)
                .await?;
        }

        Ok(())
    }

    /// Returns the number of articles stored in memory.
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

/// Helper to parse date string (YYYY-MM-DD or RFC3339).
fn parse_date_filter(s: &str, is_start: bool) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if is_start {
            return d
                .and_hms_opt(0, 0, 0)
                .map(|naive| Utc.from_utc_datetime(&naive));
        } else {
            return d
                .and_hms_opt(23, 59, 59)
                .map(|naive| Utc.from_utc_datetime(&naive));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_news_articles_registry_seeding_and_list() {
        let registry = NewsArticleRegistry::new();
        assert!(registry.count() >= 8);

        // List all
        let res = registry.list_articles(&ListNewsArticlesQuery {
            ticker: None,
            start_date: None,
            end_date: None,
            source: None,
            limit: Some(10),
            offset: Some(0),
        });
        assert_eq!(res.articles.len(), 8);
        assert_eq!(res.total, 8);
        assert!(res.articles[0].snippet.chars().count() <= 200);

        // Filter by ticker
        let aapl_res = registry.list_articles(&ListNewsArticlesQuery {
            ticker: Some("aapl".to_string()),
            start_date: None,
            end_date: None,
            source: None,
            limit: Some(20),
            offset: Some(0),
        });
        assert_eq!(aapl_res.total, 2);
        for a in &aapl_res.articles {
            assert_eq!(a.ticker, "AAPL");
        }

        // Filter by source
        let wire_res = registry.list_articles(&ListNewsArticlesQuery {
            ticker: None,
            start_date: None,
            end_date: None,
            source: Some("Wire".to_string()),
            limit: Some(20),
            offset: Some(0),
        });
        assert!(wire_res.total >= 3);
    }

    #[test]
    fn test_get_article_by_id_and_404() {
        let registry = NewsArticleRegistry::new();
        let list = registry.list_articles(&ListNewsArticlesQuery {
            ticker: None,
            start_date: None,
            end_date: None,
            source: None,
            limit: Some(1),
            offset: Some(0),
        });
        assert_eq!(list.articles.len(), 1);
        let article_id = list.articles[0].id;

        let full = registry
            .get_article(&article_id)
            .expect("Article must exist");
        assert_eq!(full.id, article_id);
        assert!(!full.full_text.is_empty());
        assert!(full.full_text.len() > full.to_metadata().snippet.len());

        let non_existent = Uuid::new_v4();
        assert!(registry.get_article(&non_existent).is_none());
    }

    #[test]
    fn test_pagination_and_sorting() {
        let registry = NewsArticleRegistry::new();
        let p1 = registry.list_articles(&ListNewsArticlesQuery {
            ticker: None,
            start_date: None,
            end_date: None,
            source: None,
            limit: Some(3),
            offset: Some(0),
        });
        assert_eq!(p1.articles.len(), 3);

        let p2 = registry.list_articles(&ListNewsArticlesQuery {
            ticker: None,
            start_date: None,
            end_date: None,
            source: None,
            limit: Some(3),
            offset: Some(3),
        });
        assert_eq!(p2.articles.len(), 3);
        assert_ne!(p1.articles[0].id, p2.articles[0].id);

        // Verify sorted descending
        for i in 0..p1.articles.len() - 1 {
            assert!(p1.articles[i].published_utc >= p1.articles[i + 1].published_utc);
        }
    }
}
