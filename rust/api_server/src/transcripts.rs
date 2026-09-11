//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Earnings Call Transcript Storage Registry
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::models::{
    TranscriptListParams, TranscriptListResponse, TranscriptMetadata, TranscriptResponse,
};

/// Stored earnings call transcript record.
#[derive(Debug, Clone)]
pub struct StoredTranscript {
    pub id: Uuid,
    pub user_id: String,
    pub ticker: String,
    pub quarter: Option<u8>,
    pub year: Option<i32>,
    pub call_date: Option<String>,
    pub transcript_text: String,
    pub source: String,
    pub sentiment_score: Option<f64>,
    pub sentiment_label: Option<String>,
    pub confidence: Option<f64>,
    pub word_count: usize,
    pub created_at: DateTime<Utc>,
}

impl StoredTranscript {
    pub fn to_metadata(&self) -> TranscriptMetadata {
        TranscriptMetadata {
            id: self.id,
            user_id: self.user_id.clone(),
            ticker: self.ticker.clone(),
            quarter: self.quarter,
            year: self.year,
            call_date: self.call_date.clone(),
            source: self.source.clone(),
            sentiment_score: self.sentiment_score,
            sentiment_label: self.sentiment_label.clone(),
            confidence: self.confidence,
            word_count: self.word_count,
            created_at: self.created_at,
        }
    }

    pub fn to_response(&self) -> TranscriptResponse {
        TranscriptResponse {
            id: self.id,
            user_id: self.user_id.clone(),
            ticker: self.ticker.clone(),
            quarter: self.quarter,
            year: self.year,
            call_date: self.call_date.clone(),
            transcript_text: self.transcript_text.clone(),
            source: self.source.clone(),
            sentiment_score: self.sentiment_score,
            sentiment_label: self.sentiment_label.clone(),
            confidence: self.confidence,
            word_count: self.word_count,
            created_at: self.created_at,
        }
    }
}

/// In-memory and PostgreSQL synchronized Earnings Call Transcript registry.
#[derive(Debug, Clone, Default)]
pub struct TranscriptRegistry {
    entries: Arc<DashMap<Uuid, StoredTranscript>>,
}

impl TranscriptRegistry {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    /// Initializes PostgreSQL schema for earnings call transcripts.
    pub async fn init_db(pool: &PgPool) -> Result<(), sqlx::Error> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS earnings_call_transcripts (
                id UUID PRIMARY KEY,
                user_id TEXT NOT NULL,
                ticker TEXT NOT NULL,
                quarter SMALLINT,
                year INT,
                call_date TEXT,
                transcript_text TEXT NOT NULL,
                source TEXT DEFAULT 'manual',
                sentiment_score FLOAT8,
                sentiment_label TEXT,
                confidence FLOAT8,
                word_count INT NOT NULL DEFAULT 0,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            CREATE INDEX IF NOT EXISTS idx_transcripts_ticker_date ON earnings_call_transcripts(ticker, call_date);
            CREATE INDEX IF NOT EXISTS idx_transcripts_user ON earnings_call_transcripts(user_id);
        "#;
        sqlx::raw_sql(sql).execute(pool).await?;
        info!("[TranscriptRegistry] Initialized PostgreSQL earnings_call_transcripts schema");
        Ok(())
    }

    /// Loads transcripts from PostgreSQL on server startup.
    pub async fn load_from_db(&self, pool: &PgPool) -> Result<usize, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, ticker, quarter, year, call_date, transcript_text,
                   source, sentiment_score, sentiment_label, confidence, word_count, created_at
            FROM earnings_call_transcripts
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(pool)
        .await?;

        let count = rows.len();
        for r in rows {
            let record = StoredTranscript {
                id: r.try_get("id")?,
                user_id: r.try_get("user_id")?,
                ticker: r.try_get("ticker")?,
                quarter: r.try_get::<Option<i16>, _>("quarter")?.map(|q| q as u8),
                year: r.try_get("year")?,
                call_date: r.try_get("call_date")?,
                transcript_text: r.try_get("transcript_text")?,
                source: r
                    .try_get::<Option<String>, _>("source")?
                    .unwrap_or_else(|| "manual".to_string()),
                sentiment_score: r.try_get("sentiment_score")?,
                sentiment_label: r.try_get("sentiment_label")?,
                confidence: r.try_get("confidence")?,
                word_count: r.try_get::<i32, _>("word_count")? as usize,
                created_at: r.try_get("created_at")?,
            };
            self.entries.insert(record.id, record);
        }

        info!(
            "[TranscriptRegistry] Loaded {} transcripts from PostgreSQL",
            count
        );
        Ok(count)
    }

    /// Inserts a new transcript into memory and optionally writes through to PostgreSQL.
    pub async fn insert(
        &self,
        transcript: StoredTranscript,
        db_pool: Option<&PgPool>,
    ) -> Result<TranscriptResponse, String> {
        let resp = transcript.to_response();
        let id = transcript.id;

        if let Some(pool) = db_pool {
            let quarter_i16 = transcript.quarter.map(|q| q as i16);
            let word_count_i32 = transcript.word_count as i32;

            let result = sqlx::query(
                r#"
                INSERT INTO earnings_call_transcripts (
                    id, user_id, ticker, quarter, year, call_date, transcript_text,
                    source, sentiment_score, sentiment_label, confidence, word_count, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                "#,
            )
            .bind(transcript.id)
            .bind(&transcript.user_id)
            .bind(&transcript.ticker)
            .bind(quarter_i16)
            .bind(transcript.year)
            .bind(&transcript.call_date)
            .bind(&transcript.transcript_text)
            .bind(&transcript.source)
            .bind(transcript.sentiment_score)
            .bind(&transcript.sentiment_label)
            .bind(transcript.confidence)
            .bind(word_count_i32)
            .bind(transcript.created_at)
            .execute(pool)
            .await;

            if let Err(e) = result {
                error!("Failed to persist transcript to PostgreSQL: {}", e);
            }
        }

        self.entries.insert(id, transcript);
        Ok(resp)
    }

    /// Lists transcript metadata for a specific user with query filtering and pagination.
    pub fn list(&self, user_id: &str, params: &TranscriptListParams) -> TranscriptListResponse {
        let limit = params.limit.unwrap_or(20).clamp(1, 100);
        let offset = params.offset.unwrap_or(0);

        let mut filtered: Vec<StoredTranscript> = self
            .entries
            .iter()
            .filter(|entry| {
                let rec = entry.value();
                // Multi-tenant user isolation: must match user_id
                if rec.user_id != user_id {
                    return false;
                }

                // Ticker filter
                if let Some(ref t) = params.ticker {
                    if !rec.ticker.eq_ignore_ascii_case(t.trim()) {
                        return false;
                    }
                }

                // Quarter filter
                if let Some(q) = params.quarter {
                    if rec.quarter != Some(q) {
                        return false;
                    }
                }

                // Year filter
                if let Some(y) = params.year {
                    if rec.year != Some(y) {
                        return false;
                    }
                }

                // Date range filter
                if let Some(ref call_date) = rec.call_date {
                    if let Some(ref start) = params.start_date {
                        if call_date < start {
                            return false;
                        }
                    }
                    if let Some(ref end) = params.end_date {
                        if call_date > end {
                            return false;
                        }
                    }
                } else if params.start_date.is_some() || params.end_date.is_some() {
                    return false;
                }

                true
            })
            .map(|entry| entry.value().clone())
            .collect();

        // Sort descending by call_date or created_at
        filtered.sort_by(|a, b| {
            b.call_date
                .cmp(&a.call_date)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        let total = filtered.len();
        let paged_items: Vec<TranscriptMetadata> = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|t| t.to_metadata())
            .collect();

        TranscriptListResponse {
            total,
            count: paged_items.len(),
            limit,
            offset,
            items: paged_items,
        }
    }

    /// Retrieves a single transcript by ID (with user isolation).
    pub fn get(&self, user_id: &str, id: Uuid) -> Option<TranscriptResponse> {
        self.entries.get(&id).and_then(|entry| {
            let rec = entry.value();
            if rec.user_id == user_id {
                Some(rec.to_response())
            } else {
                None
            }
        })
    }

    /// Deletes a single transcript by ID (with user isolation).
    pub async fn delete(
        &self,
        user_id: &str,
        id: Uuid,
        db_pool: Option<&PgPool>,
    ) -> Result<bool, String> {
        let exists_and_owned = match self.entries.get(&id) {
            Some(entry) if entry.value().user_id == user_id => true,
            _ => false,
        };

        if !exists_and_owned {
            return Ok(false);
        }

        self.entries.remove(&id);

        if let Some(pool) = db_pool {
            let _ =
                sqlx::query("DELETE FROM earnings_call_transcripts WHERE id = $1 AND user_id = $2")
                    .bind(id)
                    .bind(user_id)
                    .execute(pool)
                    .await
                    .map_err(|e| warn!("Failed to delete transcript from PostgreSQL: {}", e));
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transcript_registry_crud_and_filtering() {
        let registry = TranscriptRegistry::new();
        let user_a = "quant_fund_a";
        let user_b = "quant_fund_b";

        let t1 = StoredTranscript {
            id: Uuid::new_v4(),
            user_id: user_a.to_string(),
            ticker: "AAPL".to_string(),
            quarter: Some(4),
            year: Some(2024),
            call_date: Some("2024-10-31".to_string()),
            transcript_text: "Apple Q4 2024 record performance across all service lines..."
                .to_string(),
            source: "manual".to_string(),
            sentiment_score: Some(0.75),
            sentiment_label: Some("BULLISH".to_string()),
            confidence: Some(0.91),
            word_count: 8,
            created_at: Utc::now(),
        };
        let t1_id = t1.id;

        let t2 = StoredTranscript {
            id: Uuid::new_v4(),
            user_id: user_a.to_string(),
            ticker: "NVDA".to_string(),
            quarter: Some(3),
            year: Some(2024),
            call_date: Some("2024-11-20".to_string()),
            transcript_text: "NVIDIA Q3 2024 Blackwell demand exceeds supply...".to_string(),
            source: "whisper_asr".to_string(),
            sentiment_score: Some(0.85),
            sentiment_label: Some("BULLISH".to_string()),
            confidence: Some(0.95),
            word_count: 7,
            created_at: Utc::now(),
        };

        let t_other_user = StoredTranscript {
            id: Uuid::new_v4(),
            user_id: user_b.to_string(),
            ticker: "AAPL".to_string(),
            quarter: Some(4),
            year: Some(2024),
            call_date: Some("2024-10-31".to_string()),
            transcript_text: "Other fund transcript copy...".to_string(),
            source: "manual".to_string(),
            sentiment_score: Some(0.70),
            sentiment_label: Some("BULLISH".to_string()),
            confidence: Some(0.88),
            word_count: 4,
            created_at: Utc::now(),
        };

        registry.insert(t1, None).await.unwrap();
        registry.insert(t2, None).await.unwrap();
        registry.insert(t_other_user, None).await.unwrap();

        // 1. List user A transcripts
        let list_all = registry.list(
            user_a,
            &TranscriptListParams {
                ticker: None,
                start_date: None,
                end_date: None,
                quarter: None,
                year: None,
                limit: None,
                offset: None,
            },
        );
        assert_eq!(list_all.total, 2);
        assert_eq!(list_all.count, 2);

        // 2. Filter by ticker AAPL
        let list_aapl = registry.list(
            user_a,
            &TranscriptListParams {
                ticker: Some("AAPL".to_string()),
                start_date: None,
                end_date: None,
                quarter: None,
                year: None,
                limit: None,
                offset: None,
            },
        );
        assert_eq!(list_aapl.total, 1);
        assert_eq!(list_aapl.items[0].ticker, "AAPL");

        // 3. User isolation on GET
        assert!(registry.get(user_a, t1_id).is_some());
        assert!(registry.get(user_b, t1_id).is_none());

        // 4. Delete user A transcript
        let del_res = registry.delete(user_a, t1_id, None).await.unwrap();
        assert!(del_res);
        assert!(registry.get(user_a, t1_id).is_none());
    }
}
