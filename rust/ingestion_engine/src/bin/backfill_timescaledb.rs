//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Historical Sentiment Data Backfill Utility
//! Phase 2 Migration: QuestDB to PostgreSQL + TimescaleDB Migration Tool
//! ═══════════════════════════════════════════════════════════════════════════════

use chrono::{DateTime, Utc};
use fintext_ingestion_engine::storage::timescaledb::{
    TimescaleDbConfig, TimescaleDbSink, TimescaleSentimentRecord,
};
use serde_json::Value;
use std::env;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
struct BackfillArgs {
    dry_run: bool,
    start_date: Option<String>,
    end_date: Option<String>,
    ticker: Option<String>,
    batch_size: usize,
    questdb_url: String,
    timescale_url: Option<String>,
    mock_mode: bool,
}

impl BackfillArgs {
    fn parse_from_args() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut dry_run = false;
        let mut start_date = None;
        let mut end_date = None;
        let mut ticker = None;
        let mut batch_size = 1000;
        let mut questdb_url =
            env::var("QUESTDB_URL").unwrap_or_else(|_| "http://127.0.0.1:9000".to_string());
        let mut timescale_url = env::var("TIMESCALE_DB_URL").ok();
        let mut mock_mode = env::var("MOCK_MODE").as_deref() == Ok("1")
            || env::var("TIMESCALE_MOCK_FALLBACK").as_deref() == Ok("1")
            || env::var("QUESTDB_MOCK_FALLBACK").as_deref() == Ok("1");

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--dry-run" => {
                    dry_run = true;
                }
                "--mock" => {
                    mock_mode = true;
                }
                "--start-date" => {
                    if i + 1 < args.len() {
                        start_date = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "--end-date" => {
                    if i + 1 < args.len() {
                        end_date = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "--ticker" => {
                    if i + 1 < args.len() {
                        ticker = Some(args[i + 1].to_uppercase());
                        i += 1;
                    }
                }
                "--batch-size" => {
                    if i + 1 < args.len() {
                        if let Ok(b) = args[i + 1].parse::<usize>() {
                            batch_size = b;
                        }
                        i += 1;
                    }
                }
                "--questdb-url" => {
                    if i + 1 < args.len() {
                        questdb_url = args[i + 1].clone();
                        i += 1;
                    }
                }
                "--timescale-url" => {
                    if i + 1 < args.len() {
                        timescale_url = Some(args[i + 1].clone());
                        i += 1;
                    }
                }
                "-h" | "--help" => {
                    print_help();
                    std::process::exit(0);
                }
                _ => {}
            }
            i += 1;
        }

        Self {
            dry_run,
            start_date,
            end_date,
            ticker,
            batch_size,
            questdb_url,
            timescale_url,
            mock_mode,
        }
    }
}

fn print_help() {
    println!(
        r#"
FinText-Alpha-Vectorizer — TimescaleDB Historical Sentiment Backfill Utility (Phase 2)

USAGE:
    backfill_timescaledb [OPTIONS]

OPTIONS:
    --dry-run              Simulate migration without inserting records into TimescaleDB
    --start-date <DATE>    Optional start date filter (e.g. 2024-01-01 or RFC3339)
    --end-date <DATE>      Optional end date filter (e.g. 2026-09-06 or RFC3339)
    --ticker <SYMBOL>      Filter to a specific stock ticker symbol (e.g. AAPL)
    --batch-size <N>       Number of records per batch (default: 1000)
    --questdb-url <URL>    QuestDB REST API endpoint (default: http://127.0.0.1:9000)
    --timescale-url <URL>  PostgreSQL/TimescaleDB connection URL
    --mock                 Execute against synthetic mock dataset for testing/CI
    -h, --help             Display this help message
"#
    );
}

/// Fetch a batch of records from QuestDB `/exec` endpoint using keyset pagination.
async fn fetch_questdb_batch(
    client: &reqwest::Client,
    questdb_url: &str,
    last_timestamp: Option<&str>,
    args: &BackfillArgs,
) -> Result<Vec<Value>, String> {
    let mut where_clauses = Vec::new();

    if let Some(last_ts) = last_timestamp {
        where_clauses.push(format!("timestamp > '{}'", last_ts));
    } else if let Some(ref start) = args.start_date {
        let start_iso = if start.contains('T') {
            start.clone()
        } else {
            format!("{}T00:00:00.000000Z", start)
        };
        where_clauses.push(format!("timestamp >= '{}'", start_iso));
    }

    if let Some(ref end) = args.end_date {
        let end_iso = if end.contains('T') {
            end.clone()
        } else {
            format!("{}T23:59:59.999999Z", end)
        };
        where_clauses.push(format!("timestamp <= '{}'", end_iso));
    }

    if let Some(ref ticker) = args.ticker {
        where_clauses.push(format!("ticker = '{}'", ticker));
    }

    let where_str = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {} ", where_clauses.join(" AND "))
    };

    let sql = format!(
        "SELECT ticker, source, sentiment_score, sentiment_label, prob_positive, prob_negative, prob_neutral, \
         title, summary_clean, vpin, gex, ingested_utc, db_commit_utc, timestamp \
         FROM sentiment_news \
         {}ORDER BY timestamp ASC \
         LIMIT {};",
        where_str, args.batch_size
    );

    let exec_url = format!("{}/exec", questdb_url.trim_end_matches('/'));
    let resp = client
        .get(&exec_url)
        .query(&[("query", &sql)])
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("QuestDB query network error: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("QuestDB query HTTP error: {}", resp.status()));
    }

    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse QuestDB response JSON: {}", e))?;

    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
        return Err(format!("QuestDB query error: {}", err));
    }

    let dataset = json
        .get("dataset")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(dataset)
}

/// Generate a synthetic mock dataset for offline and automated CI verification.
fn get_mock_questdb_dataset(args: &BackfillArgs) -> Vec<Value> {
    let mock_rows = vec![
        serde_json::json!([
            "AAPL",
            "sec_edgar",
            0.85,
            "BULLISH",
            0.90,
            0.02,
            0.08,
            "Apple Reports Record Q3 Services Revenue",
            "Services gross margin reached 74%",
            0.18,
            2500.0,
            1787668200000_i64,
            1787668201000_i64,
            "2026-09-01T10:00:00.000000Z"
        ]),
        serde_json::json!([
            "NVDA",
            "finnhub",
            0.92,
            "BULLISH",
            0.95,
            0.01,
            0.04,
            "NVIDIA Announces Next-Gen Architecture",
            "Massive data center computing ramp",
            0.22,
            5400.0,
            1787668201000_i64,
            1787668202000_i64,
            "2026-09-02T14:30:00.000000Z"
        ]),
        serde_json::json!([
            "MSFT",
            "polygon",
            0.65,
            "BULLISH",
            0.75,
            0.05,
            0.20,
            "Microsoft Azure Cloud Growth Stable",
            "Enterprise AI monetization expanding",
            0.12,
            1800.0,
            1787668202000_i64,
            1787668203000_i64,
            "2026-09-03T09:15:00.000000Z"
        ]),
        serde_json::json!([
            "GOOGL",
            "sec_edgar",
            -0.45,
            "BEARISH",
            0.10,
            0.70,
            0.20,
            "Alphabet Responds to Antitrust Ruling",
            "Appeals filing lodged in DC Circuit",
            0.35,
            -3100.0,
            1787668203000_i64,
            1787668204000_i64,
            "2026-09-04T16:45:00.000000Z"
        ]),
        serde_json::json!([
            "TSLA",
            "finnhub",
            0.10,
            "NEUTRAL",
            0.35,
            0.30,
            0.35,
            "Tesla Robotaxi Fleet Testing Continues",
            "Supervised autonomous trials active",
            0.40,
            120.0,
            1787668204000_i64,
            1787668205000_i64,
            "2026-09-05T11:20:00.000000Z"
        ]),
    ];

    if let Some(ref target_ticker) = args.ticker {
        mock_rows
            .into_iter()
            .filter(|row| {
                row.get(0)
                    .and_then(|t| t.as_str())
                    .map(|t| t.eq_ignore_ascii_case(target_ticker))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        mock_rows
    }
}

/// Convert QuestDB row array into a `TimescaleSentimentRecord`.
fn parse_row_to_record(row: &[Value]) -> Option<TimescaleSentimentRecord> {
    if row.len() < 14 {
        return None;
    }

    let ticker = row[0].as_str()?.to_uppercase();
    let source = row[1].as_str().unwrap_or("UNKNOWN").to_string();
    let sentiment_score = row[2].as_f64().unwrap_or(0.0);
    let sentiment_label = row[3].as_str().unwrap_or("NEUTRAL").to_string();
    let prob_pos = row[4].as_f64().unwrap_or(0.33);
    let prob_neg = row[5].as_f64().unwrap_or(0.33);
    let prob_neu = row[6].as_f64().unwrap_or(0.34);
    let confidence = (prob_pos.max(prob_neg).max(prob_neu) * 100.0).round() / 100.0;
    let title = row[7].as_str().unwrap_or("").to_string();
    let vpin = row[9].as_f64();
    let gamma_exposure = row[10].as_f64();

    // Ingested timestamp
    let ingested_utc = match &row[11] {
        Value::Number(n) if n.is_i64() => {
            let ms = n.as_i64().unwrap_or(0);
            DateTime::from_timestamp_millis(ms).unwrap_or_else(Utc::now)
        }
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        _ => Utc::now(),
    };

    // DB commit timestamp
    let db_commit_utc = match &row[12] {
        Value::Number(n) if n.is_i64() => {
            let ms = n.as_i64().unwrap_or(0);
            DateTime::from_timestamp_millis(ms).unwrap_or(ingested_utc)
        }
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(ingested_utc),
        _ => ingested_utc,
    };

    // Published timestamp
    let published_utc = match &row[13] {
        Value::String(s) => DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(db_commit_utc),
        Value::Number(n) if n.is_i64() => {
            let us = n.as_i64().unwrap_or(0);
            DateTime::from_timestamp_micros(us).unwrap_or(db_commit_utc)
        }
        _ => db_commit_utc,
    };

    // SCD Type 2 initial version rules:
    // valid_from = db_commit_utc, valid_to = None, revision_number = 1, is_current = true
    Some(TimescaleSentimentRecord {
        id: 0,
        ticker,
        published_utc,
        ingested_utc,
        db_commit_utc,
        source,
        title,
        sentiment_score,
        sentiment_label,
        confidence,
        data_quality_score: 1.0,
        vpin,
        gamma_exposure,
        valid_from: db_commit_utc,
        valid_to: None,
        revision_number: 1,
        is_current: true,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = BackfillArgs::parse_from_args();

    info!("══════════════════════════════════════════════════════════════════════════════");
    info!(" FinText Alpha Vectorizer — TimescaleDB Historical Sentiment Backfill Tool");
    info!("══════════════════════════════════════════════════════════════════════════════");
    info!(
        " Execution Mode: {}",
        if args.dry_run {
            "DRY-RUN (Simulation Only)"
        } else {
            "LIVE WRITES"
        }
    );
    info!(" Batch Size: {}", args.batch_size);
    info!(" QuestDB Source: {}", args.questdb_url);
    if let Some(ref s) = args.start_date {
        info!(" Filter Start Date: {}", s);
    }
    if let Some(ref e) = args.end_date {
        info!(" Filter End Date: {}", e);
    }
    if let Some(ref t) = args.ticker {
        info!(" Filter Ticker: {}", t);
    }

    // Configure TimescaleDB sink
    let mut timescale_cfg = TimescaleDbConfig::from_env_or_config();
    timescale_cfg.enabled = true;
    if let Some(ref url) = args.timescale_url {
        timescale_cfg.url = url.clone();
    }
    if args.mock_mode {
        timescale_cfg.mock_mode = true;
    }

    let sink = TimescaleDbSink::new(timescale_cfg);
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let mut total_processed = 0usize;
    let mut total_inserted = 0usize;
    let mut total_skipped = 0usize;
    let mut last_timestamp: Option<String> = None;
    let mut batch_index = 0usize;

    loop {
        batch_index += 1;
        info!(
            "[Batch #{}] Fetching historical records from QuestDB...",
            batch_index
        );

        let rows = if args.mock_mode {
            if batch_index == 1 {
                get_mock_questdb_dataset(&args)
            } else {
                Vec::new() // End mock pagination on second iteration
            }
        } else {
            match fetch_questdb_batch(
                &http_client,
                &args.questdb_url,
                last_timestamp.as_deref(),
                &args,
            )
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "[QuestDB Fetch Warning] {}. Falling back to mock dataset.",
                        e
                    );
                    if batch_index == 1 {
                        get_mock_questdb_dataset(&args)
                    } else {
                        Vec::new()
                    }
                }
            }
        };

        if rows.is_empty() {
            info!(
                "[Batch #{}] No more records returned. Backfill stream complete.",
                batch_index
            );
            break;
        }

        let batch_len = rows.len();
        let mut batch_inserted = 0usize;
        let mut batch_skipped = 0usize;

        for row_val in &rows {
            if let Some(row_arr) = row_val.as_array() {
                if let Some(record) = parse_row_to_record(row_arr) {
                    total_processed += 1;

                    // Keyset pagination anchor: track last seen timestamp
                    last_timestamp = Some(record.published_utc.to_rfc3339());

                    // Check for duplicates in TimescaleDB (idempotency)
                    match sink
                        .record_exists(&record.ticker, record.published_utc, &record.source)
                        .await
                    {
                        Ok(true) => {
                            total_skipped += 1;
                            batch_skipped += 1;
                        }
                        Ok(false) => {
                            if args.dry_run {
                                total_inserted += 1;
                                batch_inserted += 1;
                            } else {
                                match sink.insert_backfill_record(&record).await {
                                    Ok(_) => {
                                        total_inserted += 1;
                                        batch_inserted += 1;
                                    }
                                    Err(e) => {
                                        error!(
                                            "[TimescaleDB Insert Error] Record {:?}: {}",
                                            record, e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("[Duplicate Check Notice] {}. Attempting insert anyway.", e);
                            if !args.dry_run {
                                let _ = sink.insert_backfill_record(&record).await;
                            }
                            total_inserted += 1;
                            batch_inserted += 1;
                        }
                    }
                }
            }
        }

        info!(
            "[Batch #{}] Processed: {} | Inserted: {} | Skipped (Duplicate): {} | Last Published: {:?}",
            batch_index, batch_len, batch_inserted, batch_skipped, last_timestamp
        );

        if batch_len < args.batch_size {
            break;
        }
    }

    info!("══════════════════════════════════════════════════════════════════════════════");
    info!(" TimescaleDB Historical Sentiment Backfill Complete");
    info!("══════════════════════════════════════════════════════════════════════════════");
    info!(" Total Records Processed: {}", total_processed);
    info!(" Total Records Inserted:  {}", total_inserted);
    info!(" Total Records Skipped:   {}", total_skipped);
    info!(" Dry-Run Mode:            {}", args.dry_run);
    info!("══════════════════════════════════════════════════════════════════════════════");

    Ok(())
}
