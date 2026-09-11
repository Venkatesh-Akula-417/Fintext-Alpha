//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Alpha Validation Report & Strategy Performance Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::auth::Claims;
use crate::handlers::backtest::{generate_mock_backtest_events, generate_mock_stock_prices};
use crate::models::{
    AlphaReportRequest, AlphaReportResponse, AlphaSignalConfig, EquityCurvePoint,
    PerformanceMetrics,
};
use crate::pit::GLOBAL_PIT_DATA;
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::Extension;
use chrono::{Duration, NaiveDate, Utc};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Default turnover fee rate (5 basis points = 0.0005)
pub const DEFAULT_TRANSACTION_COST_BPS: f64 = 5.0;

/// Execute Point-in-Time Alpha Strategy Simulation & Generate Performance Report.
///
/// Accepts strategy signal definitions, evaluates sentiment scores against threshold rules,
/// simulates equal-weighted constituent portfolio returns with turnover transaction costs,
/// and computes comprehensive institutional risk/return metrics with benchmark attribution (SPY).
#[utoipa::path(
    post,
    path = "/signals/alpha-report",
    tag = "Quantitative Research",
    request_body = AlphaReportRequest,
    responses(
        (status = 200, description = "Alpha validation report and equity curve generated successfully", body = AlphaReportResponse),
        (status = 400, description = "Invalid request parameters (invalid dates, inverted range, thresholds, or tickers)", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn post_alpha_report_handler(
    Extension(_claims): Extension<Claims>,
    Json(req): Json<AlphaReportRequest>,
) -> Response {
    // 1. Validate & Sanitize Constituent Tickers (1 to 10 tickers)
    if req.tickers.is_empty() {
        let err_body = serde_json::json!({
            "error": "Field 'tickers' must contain at least one valid ticker symbol",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    if req.tickers.len() > 10 {
        let err_body = serde_json::json!({
            "error": format!("Requested portfolio size ({} tickers) exceeds maximum allowed limit of 10 tickers", req.tickers.len()),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    let mut tickers = Vec::with_capacity(req.tickers.len());
    let mut seen = HashSet::new();
    for raw in &req.tickers {
        let clean = raw.trim().to_uppercase();
        if clean.is_empty() {
            continue;
        }
        if !seen.insert(clean.clone()) {
            continue;
        }
        match QuestDbClient::validate_and_escape_ticker(&clean) {
            Ok(valid) => tickers.push(valid),
            Err(e) => {
                error!("Invalid alpha-report ticker parameter '{}': {}", raw, e);
                let err_body = serde_json::json!({
                    "error": format!("Invalid ticker symbol '{}': {}", raw, e),
                    "status": "bad_request"
                });
                return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
            }
        }
    }

    if tickers.is_empty() {
        let err_body = serde_json::json!({
            "error": "No valid constituent ticker symbols provided",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    // 2. Validate Benchmark Ticker
    let benchmark_raw = req
        .benchmark_ticker
        .as_deref()
        .unwrap_or("SPY")
        .trim()
        .to_uppercase();
    let benchmark_ticker = match QuestDbClient::validate_and_escape_ticker(&benchmark_raw) {
        Ok(t) => t,
        Err(_) => "SPY".to_string(),
    };

    // 3. Validate Date Range
    let start_date = match NaiveDate::parse_from_str(req.start_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err_body = serde_json::json!({
                "error": format!("Invalid start_date format '{}', expected YYYY-MM-DD: {}", req.start_date, e),
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
    };

    let end_date = match NaiveDate::parse_from_str(req.end_date.trim(), "%Y-%m-%d") {
        Ok(d) => d,
        Err(e) => {
            let err_body = serde_json::json!({
                "error": format!("Invalid end_date format '{}', expected YYYY-MM-DD: {}", req.end_date, e),
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
    };

    if start_date > end_date {
        let err_body = serde_json::json!({
            "error": format!("start_date ({}) cannot be after end_date ({})", req.start_date, req.end_date),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    let total_days = (end_date - start_date).num_days();
    if total_days > 3650 {
        let err_body = serde_json::json!({
            "error": "Requested evaluation horizon exceeds maximum allowed limit of 10 years (3650 days)",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    // 4. Validate Signal Thresholds & Configurations
    if req.signal_config.threshold_long < req.signal_config.threshold_short {
        let err_body = serde_json::json!({
            "error": format!(
                "threshold_long ({}) must be >= threshold_short ({})",
                req.signal_config.threshold_long, req.signal_config.threshold_short
            ),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    let holding_days = req.signal_config.holding_days.max(1) as i64;
    let initial_capital = req.initial_capital.unwrap_or(1_000_000.0).max(100.0);
    let smoothing_window = req
        .signal_config
        .smoothing_window_days
        .map(|w| w.clamp(1, 30) as usize);

    // 5. Point-in-Time Validation
    let pit_data = GLOBAL_PIT_DATA.clone();
    if pit_data.is_enabled() {
        for t in &tickers {
            if !pit_data.is_valid_ticker(t, start_date) {
                let err_body = serde_json::json!({
                    "error": format!("Ticker '{}' was not active or listed on start_date '{}'", t, req.start_date),
                    "status": "bad_request"
                });
                return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
            }
        }
    }

    let is_mock = crate::state::is_questdb_mock_fallback_enabled();
    let eff_start_str = start_date.format("%Y-%m-%d").to_string();
    let eff_end_str = end_date.format("%Y-%m-%d").to_string();

    // 6. Ingest Historical Sentiment Data
    let mut ticker_events: HashMap<String, Vec<(f64, i64)>> = HashMap::new();
    if is_mock {
        info!("Executing alpha report in QuestDB mock mode for tickers: {:?}", tickers);
        for t in &tickers {
            ticker_events.insert(t.clone(), generate_mock_backtest_events(t, start_date, end_date));
        }
    } else {
        for t in &tickers {
            match QUESTDB_CLIENT
                .query_sentiment_history(t, &eff_start_str, &eff_end_str)
                .await
            {
                Ok(events) => {
                    let ev = if events.is_empty() {
                        if crate::state::is_production_mode() {
                            Vec::new()
                        } else {
                            generate_mock_backtest_events(t, start_date, end_date)
                        }
                    } else {
                        events
                    };
                    ticker_events.insert(t.clone(), ev);
                }
                Err(err) => {
                    if crate::state::is_production_mode() {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({
                                "error": "Service Unavailable",
                                "message": "Required data source unavailable in production mode.",
                                "detail": format!("{}", err),
                                "status": "service_unavailable"
                            })),
                        )
                            .into_response();
                    }
                    warn!("QuestDB error querying sentiment for ticker '{}': {}. Using mock fallback.", t, err);
                    ticker_events.insert(t.clone(), generate_mock_backtest_events(t, start_date, end_date));
                }
            }
        }
    }

    // Benchmark Sentiment
    let benchmark_events = if is_mock {
        generate_mock_backtest_events(&benchmark_ticker, start_date, end_date)
    } else {
        match QUESTDB_CLIENT
            .query_sentiment_history(&benchmark_ticker, &eff_start_str, &eff_end_str)
            .await
        {
            Ok(events) => {
                if events.is_empty() {
                    if crate::state::is_production_mode() {
                        Vec::new()
                    } else {
                        generate_mock_backtest_events(&benchmark_ticker, start_date, end_date)
                    }
                } else {
                    events
                }
            }
            Err(e) => {
                if crate::state::is_production_mode() {
                    return (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(serde_json::json!({
                            "error": "Service Unavailable",
                            "message": "Required data source unavailable in production mode.",
                            "detail": format!("{}", e),
                            "status": "service_unavailable"
                        })),
                    )
                        .into_response();
                }
                generate_mock_backtest_events(&benchmark_ticker, start_date, end_date)
            }
        }
    };

    // 7. Ingest Daily Stock Prices
    let mut all_price_tickers = tickers.clone();
    if !all_price_tickers.contains(&benchmark_ticker) {
        all_price_tickers.push(benchmark_ticker.clone());
    }

    let ticker_prices = if is_mock {
        let mut prices = HashMap::new();
        for t in &all_price_tickers {
            prices.insert(t.clone(), generate_mock_stock_prices(t, start_date, end_date));
        }
        prices
    } else {
        match QUESTDB_CLIENT
            .query_stock_prices(&all_price_tickers, &eff_start_str, &eff_end_str)
            .await
        {
            Ok(prices) => prices,
            Err(err) => {
                warn!("QuestDB error querying stock prices: {}. Proceeding with sentiment-proxy returns.", err);
                HashMap::new()
            }
        }
    };

    // 8. Run Institutional Simulation
    let response = run_alpha_strategy_simulation(
        &tickers,
        &benchmark_ticker,
        start_date,
        end_date,
        &req.signal_config,
        &ticker_events,
        &benchmark_events,
        &ticker_prices,
        holding_days,
        smoothing_window,
        initial_capital,
    );

    (StatusCode::OK, Json(response)).into_response()
}

/// Executes the core alpha signal simulation state machine, smoothing, and risk attribution.
pub fn run_alpha_strategy_simulation(
    tickers: &[String],
    benchmark_ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    signal_config: &AlphaSignalConfig,
    ticker_events: &HashMap<String, Vec<(f64, i64)>>,
    benchmark_events: &[(f64, i64)],
    ticker_prices: &HashMap<String, HashMap<NaiveDate, f64>>,
    holding_days: i64,
    smoothing_window: Option<usize>,
    initial_capital: f64,
) -> AlphaReportResponse {
    let fee_rate = DEFAULT_TRANSACTION_COST_BPS / 10_000.0; // 0.0005
    let equal_weight = 1.0 / (tickers.len() as f64);

    // 1. Build dense daily average score lookup for each ticker
    let mut daily_ticker_scores: HashMap<String, HashMap<NaiveDate, f64>> = HashMap::new();
    for (t, events) in ticker_events {
        let mut daily_sums: HashMap<NaiveDate, (f64, usize)> = HashMap::new();
        for &(score, ts_us) in events {
            let secs = ts_us / 1_000_000;
            let nsecs = ((ts_us % 1_000_000) * 1_000) as u32;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                let date = dt.naive_utc().date();
                let entry = daily_sums.entry(date).or_insert((0.0, 0));
                entry.0 += score;
                entry.1 += 1;
            }
        }
        let mut avg_map = HashMap::new();
        for (d, (sum, count)) in daily_sums {
            if count > 0 {
                avg_map.insert(d, sum / count as f64);
            }
        }
        daily_ticker_scores.insert(t.clone(), avg_map);
    }

    // Benchmark daily score lookup
    let mut benchmark_daily_scores = HashMap::new();
    {
        let mut daily_sums: HashMap<NaiveDate, (f64, usize)> = HashMap::new();
        for &(score, ts_us) in benchmark_events {
            let secs = ts_us / 1_000_000;
            let nsecs = ((ts_us % 1_000_000) * 1_000) as u32;
            if let Some(dt) = chrono::DateTime::from_timestamp(secs, nsecs) {
                let date = dt.naive_utc().date();
                let entry = daily_sums.entry(date).or_insert((0.0, 0));
                entry.0 += score;
                entry.1 += 1;
            }
        }
        for (d, (sum, count)) in daily_sums {
            if count > 0 {
                benchmark_daily_scores.insert(d, sum / count as f64);
            }
        }
    }

    // 2. Build ordered calendar timeline
    let mut dates = Vec::new();
    let mut curr = start_date;
    while curr <= end_date {
        dates.push(curr);
        curr += Duration::days(1);
    }

    // 3. Compute smoothed scores if smoothing window configured
    let mut smoothed_ticker_scores: HashMap<String, HashMap<NaiveDate, f64>> = HashMap::new();
    for t in tickers {
        let raw_scores = daily_ticker_scores.get(t);
        let mut smoothed_map = HashMap::new();
        for (idx, &date) in dates.iter().enumerate() {
            if let Some(w) = smoothing_window {
                let start_idx = idx.saturating_sub(w - 1);
                let window_dates = &dates[start_idx..=idx];
                let mut sum = 0.0;
                let mut count = 0;
                for &wd in window_dates {
                    if let Some(s) = raw_scores.and_then(|m| m.get(&wd)) {
                        sum += *s;
                        count += 1;
                    }
                }
                let smoothed = if count > 0 { sum / (count as f64) } else { 0.0 };
                smoothed_map.insert(date, smoothed);
            } else {
                let s = raw_scores.and_then(|m| m.get(&date)).copied().unwrap_or(0.0);
                smoothed_map.insert(date, s);
            }
        }
        smoothed_ticker_scores.insert(t.clone(), smoothed_map);
    }

    // 4. Portfolio state machine simulation
    let n_assets = tickers.len();
    let mut positions: Vec<i32> = vec![0; n_assets]; // -1, 0, 1
    let mut days_held: Vec<i64> = vec![0; n_assets];
    let mut asset_cumulative_pnl: Vec<f64> = vec![1.0; n_assets];
    let mut closed_trade_durations: Vec<i64> = Vec::new();
    let mut closed_trade_returns: Vec<f64> = Vec::new();
    let mut total_trades_count: usize = 0;

    let mut portfolio_equity = initial_capital;
    let mut benchmark_equity = initial_capital;

    let mut equity_curve: Vec<EquityCurvePoint> = Vec::with_capacity(dates.len());
    let mut strategy_daily_returns: Vec<f64> = Vec::with_capacity(dates.len());
    let mut benchmark_daily_returns: Vec<f64> = Vec::with_capacity(dates.len());

    for &date in &dates {
        let mut daily_gross_return = 0.0;
        let mut daily_turnover = 0.0;

        for i in 0..n_assets {
            let t = &tickers[i];
            let score = smoothed_ticker_scores
                .get(t)
                .and_then(|m| m.get(&date))
                .copied()
                .unwrap_or(0.0);

            let target_pos = if score >= signal_config.threshold_long {
                1
            } else if score <= signal_config.threshold_short {
                -1
            } else if positions[i] != 0 && days_held[i] >= holding_days {
                0
            } else {
                positions[i]
            };

            let delta_pos = (target_pos - positions[i]).abs() as f64;
            if target_pos != positions[i] {
                if positions[i] != 0 {
                    let trade_ret = asset_cumulative_pnl[i] - 1.0;
                    closed_trade_returns.push(trade_ret);
                    closed_trade_durations.push(days_held[i].max(1));
                }
                positions[i] = target_pos;
                days_held[i] = 0;
                asset_cumulative_pnl[i] = 1.0;
                total_trades_count += 1;
            } else if positions[i] != 0 {
                days_held[i] += 1;
            }

            daily_turnover += equal_weight * delta_pos;

            // Compute daily asset return
            let prev_date = date - Duration::days(1);
            let price_map = ticker_prices.get(t);
            let asset_daily_ret = if let (Some(curr_p), Some(prev_p)) = (
                price_map.and_then(|m| m.get(&date)),
                price_map.and_then(|m| m.get(&prev_date)),
            ) {
                if *prev_p > 0.0 {
                    let p_ret = (*curr_p - *prev_p) / *prev_p;
                    (positions[i] as f64) * p_ret
                } else {
                    (positions[i] as f64) * score * 0.01
                }
            } else {
                (positions[i] as f64) * score * 0.01
            };

            asset_cumulative_pnl[i] *= 1.0 + asset_daily_ret;
            daily_gross_return += equal_weight * asset_daily_ret;
        }

        // Apply transaction costs
        let transaction_fee_pct = daily_turnover * fee_rate;
        let daily_net_ret = daily_gross_return - transaction_fee_pct;

        portfolio_equity *= 1.0 + daily_net_ret;
        strategy_daily_returns.push(daily_net_ret);

        // Benchmark daily return
        let prev_date = date - Duration::days(1);
        let bench_price_map = ticker_prices.get(benchmark_ticker);
        let bench_daily_ret = if let (Some(curr_bp), Some(prev_bp)) = (
            bench_price_map.and_then(|m| m.get(&date)),
            bench_price_map.and_then(|m| m.get(&prev_date)),
        ) {
            if *prev_bp > 0.0 {
                (*curr_bp - *prev_bp) / *prev_bp
            } else {
                let bench_score = benchmark_daily_scores.get(&date).copied().unwrap_or(0.0);
                0.0004 + (bench_score * 0.005)
            }
        } else {
            let bench_score = benchmark_daily_scores.get(&date).copied().unwrap_or(0.0);
            0.0004 + (bench_score * 0.005)
        };

        benchmark_equity *= 1.0 + bench_daily_ret;
        benchmark_daily_returns.push(bench_daily_ret);

        // Average portfolio position state
        let net_pos_sum: i32 = positions.iter().sum();
        let dominant_pos = if net_pos_sum > 0 {
            1
        } else if net_pos_sum < 0 {
            -1
        } else {
            0
        };

        equity_curve.push(EquityCurvePoint {
            date: date.format("%Y-%m-%d").to_string(),
            portfolio_value: (portfolio_equity * 100.0).round() / 100.0,
            benchmark_value: (benchmark_equity * 100.0).round() / 100.0,
            strategy_daily_return: (daily_net_ret * 1_000_000.0).round() / 1_000_000.0,
            benchmark_daily_return: (bench_daily_ret * 1_000_000.0).round() / 1_000_000.0,
            position: dominant_pos,
        });
    }

    // Close remaining open positions
    for i in 0..n_assets {
        if positions[i] != 0 {
            let trade_ret = asset_cumulative_pnl[i] - 1.0;
            closed_trade_returns.push(trade_ret);
            closed_trade_durations.push(days_held[i].max(1));
        }
    }

    // 5. Compute Institutional Quantitative Risk & Performance Metrics
    let n_days = strategy_daily_returns.len().max(1) as f64;
    let total_return = (portfolio_equity - initial_capital) / initial_capital;
    let annualized_return = if total_return > -1.0 {
        (1.0 + total_return).powf(252.0 / n_days) - 1.0
    } else {
        -1.0
    };

    let mean_strat_ret = strategy_daily_returns.iter().sum::<f64>() / n_days;
    let variance_strat = strategy_daily_returns
        .iter()
        .map(|&r| (r - mean_strat_ret).powi(2))
        .sum::<f64>()
        / n_days;
    let std_dev_strat = variance_strat.sqrt();
    let annualized_volatility = std_dev_strat * 252.0_f64.sqrt();

    let sharpe_ratio = if std_dev_strat > 1e-9 {
        (mean_strat_ret / std_dev_strat) * 252.0_f64.sqrt()
    } else {
        0.0
    };

    // Downside deviation & Sortino ratio
    let downside_variance = strategy_daily_returns
        .iter()
        .map(|&r| if r < 0.0 { r.powi(2) } else { 0.0 })
        .sum::<f64>()
        / n_days;
    let downside_vol = (downside_variance * 252.0).sqrt();
    let sortino_ratio = if downside_vol > 1e-9 {
        (annualized_return) / downside_vol
    } else if annualized_return > 0.0 {
        99.99
    } else {
        0.0
    };

    // Maximum drawdown
    let mut peak = initial_capital;
    let mut max_drawdown = 0.0;
    for pt in &equity_curve {
        if pt.portfolio_value > peak {
            peak = pt.portfolio_value;
        } else if peak > 0.0 {
            let dd = (peak - pt.portfolio_value) / peak;
            if dd > max_drawdown {
                max_drawdown = dd;
            }
        }
    }

    // Win Rate
    let win_rate = if !closed_trade_returns.is_empty() {
        let wins = closed_trade_returns.iter().filter(|&&r| r > 0.0).count();
        (wins as f64 / closed_trade_returns.len() as f64) * 100.0
    } else if !strategy_daily_returns.is_empty() {
        let wins = strategy_daily_returns.iter().filter(|&&r| r > 0.0).count();
        (wins as f64 / strategy_daily_returns.len() as f64) * 100.0
    } else {
        0.0
    };

    // Profit factor
    let gross_profit: f64 = strategy_daily_returns.iter().filter(|&&r| r > 0.0).sum();
    let gross_loss: f64 = strategy_daily_returns
        .iter()
        .filter(|&&r| r < 0.0)
        .map(|r| r.abs())
        .sum();
    let profit_factor = if gross_loss > 1e-9 {
        gross_profit / gross_loss
    } else if gross_profit > 1e-9 {
        99.99
    } else {
        1.0
    };

    let total_trades = if total_trades_count > 0 {
        total_trades_count
    } else {
        closed_trade_returns.len()
    };

    let avg_holding_period_days = if !closed_trade_durations.is_empty() {
        let sum_dur: i64 = closed_trade_durations.iter().sum();
        sum_dur as f64 / closed_trade_durations.len() as f64
    } else {
        holding_days as f64
    };

    // Benchmark statistics
    let benchmark_total_return = (benchmark_equity - initial_capital) / initial_capital;
    let benchmark_annualized_return = if benchmark_total_return > -1.0 {
        (1.0 + benchmark_total_return).powf(252.0 / n_days) - 1.0
    } else {
        -1.0
    };

    let mean_bench_ret = benchmark_daily_returns.iter().sum::<f64>() / n_days;
    let variance_bench = benchmark_daily_returns
        .iter()
        .map(|&r| (r - mean_bench_ret).powi(2))
        .sum::<f64>()
        / n_days;
    let benchmark_annualized_volatility = variance_bench.sqrt() * 252.0_f64.sqrt();

    // Alpha (Excess Total Return)
    let alpha = total_return - benchmark_total_return;

    // Beta (OLS slope of strategy returns vs benchmark returns)
    let cov_strat_bench = strategy_daily_returns
        .iter()
        .zip(benchmark_daily_returns.iter())
        .map(|(&rs, &rb)| (rs - mean_strat_ret) * (rb - mean_bench_ret))
        .sum::<f64>()
        / n_days;
    let beta = if variance_bench > 1e-9 {
        cov_strat_bench / variance_bench
    } else {
        1.0
    };

    // Active Return, Tracking Error & Information Ratio
    let active_returns: Vec<f64> = strategy_daily_returns
        .iter()
        .zip(benchmark_daily_returns.iter())
        .map(|(&rs, &rb)| rs - rb)
        .collect();
    let mean_active_ret = active_returns.iter().sum::<f64>() / n_days;
    let variance_active = active_returns
        .iter()
        .map(|&r| (r - mean_active_ret).powi(2))
        .sum::<f64>()
        / n_days;
    let tracking_error = variance_active.sqrt() * 252.0_f64.sqrt();
    let information_ratio = if tracking_error > 1e-9 {
        (mean_active_ret * 252.0) / tracking_error
    } else {
        0.0
    };

    let metrics = PerformanceMetrics {
        total_return: (total_return * 10_000.0).round() / 10_000.0,
        annualized_return: (annualized_return * 10_000.0).round() / 10_000.0,
        annualized_volatility: (annualized_volatility * 10_000.0).round() / 10_000.0,
        sharpe_ratio: (sharpe_ratio * 100.0).round() / 100.0,
        sortino_ratio: (sortino_ratio * 100.0).round() / 100.0,
        max_drawdown: (max_drawdown * 10_000.0).round() / 10_000.0,
        win_rate: (win_rate * 10.0).round() / 10.0,
        profit_factor: (profit_factor * 100.0).round() / 100.0,
        total_trades,
        avg_holding_period_days: (avg_holding_period_days * 10.0).round() / 10.0,
        benchmark_ticker: benchmark_ticker.to_string(),
        benchmark_total_return: (benchmark_total_return * 10_000.0).round() / 10_000.0,
        benchmark_annualized_return: (benchmark_annualized_return * 10_000.0).round() / 10_000.0,
        benchmark_annualized_volatility: (benchmark_annualized_volatility * 10_000.0).round()
            / 10_000.0,
        alpha: (alpha * 10_000.0).round() / 10_000.0,
        beta: (beta * 100.0).round() / 100.0,
        information_ratio: (information_ratio * 100.0).round() / 100.0,
        tracking_error: (tracking_error * 10_000.0).round() / 10_000.0,
    };

    let message = format!(
        "Alpha validation report generated for {} constituent tickers from {} to {} (holding_days={}, smoothing={:?})",
        tickers.len(),
        start_date,
        end_date,
        holding_days,
        smoothing_window
    );

    AlphaReportResponse {
        tickers: tickers.to_vec(),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        signal_config: signal_config.clone(),
        initial_capital,
        metrics,
        equity_curve,
        generated_at: Utc::now().to_rfc3339(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_simulation_calculations() {
        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let start_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let end_date = NaiveDate::from_ymd_opt(2024, 6, 30).unwrap();
        let signal_config = AlphaSignalConfig {
            signal_type: "sentiment".to_string(),
            threshold_long: 0.2,
            threshold_short: -0.2,
            holding_days: 5,
            smoothing_window_days: Some(3),
        };

        let mut ticker_events = HashMap::new();
        for t in &tickers {
            ticker_events.insert(t.clone(), generate_mock_backtest_events(t, start_date, end_date));
        }
        let benchmark_events = generate_mock_backtest_events("SPY", start_date, end_date);

        let mut ticker_prices = HashMap::new();
        for t in &["AAPL", "NVDA", "SPY"] {
            ticker_prices.insert(t.to_string(), generate_mock_stock_prices(t, start_date, end_date));
        }

        let resp = run_alpha_strategy_simulation(
            &tickers,
            "SPY",
            start_date,
            end_date,
            &signal_config,
            &ticker_events,
            &benchmark_events,
            &ticker_prices,
            5,
            Some(3),
            1_000_000.0,
        );

        assert_eq!(resp.tickers.len(), 2);
        assert_eq!(resp.signal_config.smoothing_window_days, Some(3));
        assert!(resp.metrics.annualized_volatility > 0.0);
        assert!(!resp.equity_curve.is_empty());
        assert_eq!(resp.metrics.benchmark_ticker, "SPY");
        assert!(resp.metrics.beta != 0.0);
    }
}
