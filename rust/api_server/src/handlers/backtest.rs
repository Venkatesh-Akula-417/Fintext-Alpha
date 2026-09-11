//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Multi-Asset Portfolio Point-in-Time Quantitative Backtesting Handler
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::models::{BacktestRequest, BacktestResponse, EquityPoint};
use crate::storage::{QuestDbClient, QuestDbClientConfig};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{Duration, NaiveDate};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use tracing::{error, info, warn};

static QUESTDB_CLIENT: Lazy<QuestDbClient> =
    Lazy::new(|| QuestDbClient::new(QuestDbClientConfig::default()));

/// Default transaction cost applied on portfolio turnover (5 basis points = 0.05%)
pub const DEFAULT_TRANSACTION_COST_BPS: f64 = 5.0;

/// Execute Point-in-Time Multi-Asset Portfolio Alpha Strategy Simulation.
///
/// Simulates an institutional Long/Short equity sentiment strategy across a single ticker or
/// multi-asset portfolio (1 to 10 tickers) with custom/equal weighting and transaction costs.
/// Calculates total returns, annualized returns, Sharpe ratio, Sortino ratio, max drawdown,
/// win rate, profit factor, benchmark comparison (SPY), and daily equity curve timeseries.
#[utoipa::path(
    post,
    path = "/backtest",
    tag = "Quantitative Backtesting",
    request_body = BacktestRequest,
    responses(
        (status = 200, description = "Multi-asset backtest simulation completed successfully", body = BacktestResponse),
        (status = 400, description = "Invalid request parameters (invalid dates, inverted range, thresholds, weights, or tickers)", body = crate::auth::AuthErrorResponse),
        (status = 401, description = "Unauthorized (missing or invalid Bearer JWT)", body = crate::auth::AuthErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::rate_limit::RateLimitErrorResponse)
    ),
    security(
        ("bearerAuth" = [])
    )
)]
pub async fn backtest_handler(Json(req): Json<BacktestRequest>) -> Response {
    // 1. Resolve Target Tickers (Support both `tickers` array and legacy `ticker` string)
    let mut raw_tickers = Vec::new();
    if let Some(ref list) = req.tickers {
        for t in list {
            let clean = t.trim().to_uppercase();
            if !clean.is_empty() {
                raw_tickers.push(clean);
            }
        }
    }
    if raw_tickers.is_empty() {
        if let Some(ref single) = req.ticker {
            let clean = single.trim().to_uppercase();
            if !clean.is_empty() {
                raw_tickers.push(clean);
            }
        }
    }

    if raw_tickers.is_empty() {
        let err_body = serde_json::json!({
            "error": "At least one ticker symbol must be specified in 'tickers' or 'ticker'",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    if raw_tickers.len() > 10 {
        let err_body = serde_json::json!({
            "error": format!("Requested portfolio size ({} tickers) exceeds maximum allowed limit of 10 tickers", raw_tickers.len()),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    // Validate ticker syntax and remove duplicates while preserving order
    let mut tickers = Vec::with_capacity(raw_tickers.len());
    let mut seen = HashSet::new();
    for t in &raw_tickers {
        if !seen.insert(t.clone()) {
            continue;
        }
        match QuestDbClient::validate_and_escape_ticker(t) {
            Ok(valid) => tickers.push(valid),
            Err(e) => {
                error!("Invalid backtest ticker parameter '{}': {}", t, e);
                let err_body = serde_json::json!({
                    "error": format!("Invalid ticker '{}': {}", t, e),
                    "status": "bad_request"
                });
                return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
            }
        }
    }

    // 2. Validate Portfolio Weights
    let weights = if let Some(ref user_weights) = req.weights {
        if user_weights.len() != tickers.len() {
            let err_body = serde_json::json!({
                "error": format!("Number of weights ({}) must match number of unique tickers ({})", user_weights.len(), tickers.len()),
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
        let mut sum = 0.0;
        for &w in user_weights {
            if w < 0.0 {
                let err_body = serde_json::json!({
                    "error": format!("Portfolio weights must be non-negative, found {}", w),
                    "status": "bad_request"
                });
                return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
            }
            sum += w;
        }
        if sum <= 0.0 || (sum - 1.0).abs() > 0.05 {
            let err_body = serde_json::json!({
                "error": format!("Portfolio weights must sum to approximately 1.0 (got sum={:.4})", sum),
                "status": "bad_request"
            });
            return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
        }
        user_weights.iter().map(|&w| w / sum).collect::<Vec<f64>>()
    } else {
        let w = 1.0 / (tickers.len() as f64);
        vec![w; tickers.len()]
    };

    // 3. Resolve Benchmark Ticker & Transaction Costs
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

    let transaction_cost_bps = req
        .transaction_cost_bps
        .unwrap_or(DEFAULT_TRANSACTION_COST_BPS);
    if transaction_cost_bps < 0.0 || transaction_cost_bps > 100.0 {
        let err_body = serde_json::json!({
            "error": format!("transaction_cost_bps ({}) must be between 0.0 and 100.0 bps", transaction_cost_bps),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    // 4. Validate Simulation Dates & Thresholds
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
            "error": "Requested backtest horizon exceeds maximum allowed limit of 10 years (3650 days)",
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    if req.long_threshold < req.short_threshold {
        let err_body = serde_json::json!({
            "error": format!("long_threshold ({}) must be >= short_threshold ({})", req.long_threshold, req.short_threshold),
            "status": "bad_request"
        });
        return (StatusCode::BAD_REQUEST, Json(err_body)).into_response();
    }

    let initial_capital = if req.initial_capital > 0.0 {
        req.initial_capital
    } else {
        1_000_000.0
    };

    let holding_days = req.holding_days.max(1);

    // 5. Point-in-Time (PIT) Validation across all constituents
    let pit_data = crate::pit::GLOBAL_PIT_DATA.clone();
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

    // 6. Ingest Historical Data for all tickers
    let mut ticker_events = HashMap::new();
    let eff_start_str = start_date.format("%Y-%m-%d").to_string();
    let eff_end_str = end_date.format("%Y-%m-%d").to_string();

    if is_mock {
        info!(
            "Executing multi-asset backtest in QuestDB mock mode for tickers: {:?}",
            tickers
        );
        for t in &tickers {
            let events = generate_mock_backtest_events(t, start_date, end_date);
            ticker_events.insert(t.clone(), events);
        }
    } else {
        for t in &tickers {
            match QUESTDB_CLIENT
                .query_sentiment_history(t, &eff_start_str, &eff_end_str)
                .await
            {
                Ok(events) => {
                    let ev_to_use = if events.is_empty() {
                        if crate::state::is_production_mode() {
                            Vec::new()
                        } else {
                            generate_mock_backtest_events(t, start_date, end_date)
                        }
                    } else {
                        events
                    };
                    ticker_events.insert(t.clone(), ev_to_use);
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
                    let ev = generate_mock_backtest_events(t, start_date, end_date);
                    ticker_events.insert(t.clone(), ev);
                }
            }
        }
    }

    // Benchmark historical data
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

    // 7. Ingest Daily Stock Prices for all tickers + benchmark
    let mut all_price_tickers = tickers.clone();
    if !all_price_tickers.contains(&benchmark_ticker) {
        all_price_tickers.push(benchmark_ticker.clone());
    }

    let ticker_prices = if is_mock {
        let mut prices = HashMap::new();
        for t in &all_price_tickers {
            prices.insert(
                t.clone(),
                generate_mock_stock_prices(t, start_date, end_date),
            );
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

    let has_actual_prices = tickers
        .iter()
        .any(|t| ticker_prices.get(t).map_or(false, |m| !m.is_empty()));
    let message = if has_actual_prices {
        "Point-in-time multi-asset backtest executed using actual OHLCV stock price returns with trading costs"
    } else if is_mock {
        "Point-in-time multi-asset backtest simulated successfully (QuestDB mock mode, sentiment proxy returns with trading costs)"
    } else {
        "Point-in-time multi-asset backtest executed against QuestDB historical records (sentiment-proxy return model with trading costs)"
    };

    // 8. Execute Multi-Asset Portfolio Simulation
    let resp = run_portfolio_simulation(
        &tickers,
        &weights,
        &benchmark_ticker,
        start_date,
        end_date,
        &ticker_events,
        &benchmark_events,
        &ticker_prices,
        req.long_threshold,
        req.short_threshold,
        holding_days,
        initial_capital,
        transaction_cost_bps,
        message,
    );

    (StatusCode::OK, Json(resp)).into_response()
}

/// Executes the multi-asset portfolio point-in-time simulation state machine with turnover transaction costs.
pub fn run_portfolio_simulation(
    tickers: &[String],
    weights: &[f64],
    benchmark_ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    ticker_events: &HashMap<String, Vec<(f64, i64)>>,
    benchmark_events: &[(f64, i64)],
    ticker_prices: &HashMap<String, HashMap<NaiveDate, f64>>,
    long_threshold: f64,
    short_threshold: f64,
    holding_days: i64,
    initial_capital: f64,
    transaction_cost_bps: f64,
    message: &str,
) -> BacktestResponse {
    let fee_rate = transaction_cost_bps / 10_000.0; // e.g. 5 bps -> 0.0005

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

    // 2. Build ordered daily timeline from start_date to end_date
    let mut dates = Vec::new();
    let mut curr = start_date;
    while curr <= end_date {
        dates.push(curr);
        curr += Duration::days(1);
    }

    let n_assets = tickers.len();
    let mut positions: Vec<i64> = vec![0; n_assets]; // 0 = flat, 1 = long, -1 = short
    let mut days_held: Vec<i64> = vec![0; n_assets];
    let mut asset_cumulative_pnl: Vec<f64> = vec![1.0; n_assets];

    let mut portfolio_equity = initial_capital;
    let mut benchmark_equity = initial_capital;

    let mut equity_curve = Vec::with_capacity(dates.len() + 1);
    let mut equity_points = Vec::with_capacity(dates.len() + 1);
    let mut benchmark_curve = Vec::with_capacity(dates.len() + 1);
    let mut daily_net_returns = Vec::with_capacity(dates.len());
    let mut closed_trade_returns = Vec::new();
    let mut total_trades_count: usize = 0;

    equity_curve.push(portfolio_equity);
    benchmark_curve.push(benchmark_equity);
    equity_points.push(EquityPoint {
        date: start_date.format("%Y-%m-%d").to_string(),
        portfolio_value: portfolio_equity,
        daily_return: 0.0,
    });

    for &date in &dates {
        let mut daily_gross_return = 0.0;
        let mut daily_turnover = 0.0;

        for i in 0..n_assets {
            let t = &tickers[i];
            let w = weights[i];
            let score = daily_ticker_scores
                .get(t)
                .and_then(|m| m.get(&date))
                .copied()
                .unwrap_or(0.0);

            // Determine target position based on sentiment thresholds
            let target_pos = if score >= long_threshold {
                1
            } else if score <= short_threshold {
                -1
            } else if positions[i] != 0 && days_held[i] >= holding_days {
                0 // holding duration expired -> return to flat
            } else {
                positions[i] // maintain position
            };

            // Transition handling
            let delta_pos = (target_pos - positions[i]).abs() as f64;
            if target_pos != positions[i] {
                if positions[i] != 0 {
                    // Record closed trade return
                    let trade_ret = asset_cumulative_pnl[i] - 1.0;
                    closed_trade_returns.push(trade_ret);
                }
                positions[i] = target_pos;
                days_held[i] = 0;
                asset_cumulative_pnl[i] = 1.0;
                total_trades_count += 1;
            } else if positions[i] != 0 {
                days_held[i] += 1;
            }

            daily_turnover += w * delta_pos;

            // Daily asset return (Actual stock price return if available, else sentiment proxy model)
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
            daily_gross_return += w * asset_daily_ret;
        }

        // Apply turnover transaction cost
        let transaction_fee_pct = daily_turnover * fee_rate;
        let daily_net_ret = daily_gross_return - transaction_fee_pct;

        portfolio_equity *= 1.0 + daily_net_ret;
        daily_net_returns.push(daily_net_ret);

        let rounded_eq = (portfolio_equity * 100.0).round() / 100.0;
        equity_curve.push(rounded_eq);
        equity_points.push(EquityPoint {
            date: date.format("%Y-%m-%d").to_string(),
            portfolio_value: rounded_eq,
            daily_return: (daily_net_ret * 10000.0).round() / 10000.0,
        });

        // Benchmark update (actual benchmark price return if available, else SPY drift + sentiment proxy)
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
        benchmark_curve.push((benchmark_equity * 100.0).round() / 100.0);
    }

    // Close any remaining open positions
    for i in 0..n_assets {
        if positions[i] != 0 {
            let trade_ret = asset_cumulative_pnl[i] - 1.0;
            closed_trade_returns.push(trade_ret);
        }
    }

    // 4. Compute Institutional Risk & Performance Metrics
    let total_return = (portfolio_equity - initial_capital) / initial_capital;
    let n_days = daily_net_returns.len().max(1) as f64;

    let annualized_return = if total_return > -1.0 {
        (1.0 + total_return).powf(252.0 / n_days) - 1.0
    } else {
        -1.0
    };

    let mean_ret = daily_net_returns.iter().sum::<f64>() / n_days;
    let variance = daily_net_returns
        .iter()
        .map(|&r| (r - mean_ret).powi(2))
        .sum::<f64>()
        / n_days;
    let std_dev = variance.sqrt();

    let sharpe_ratio = if std_dev > 1e-9 {
        (mean_ret / std_dev) * 252.0_f64.sqrt()
    } else {
        0.0
    };

    // Sortino Ratio (Downside deviation, MAR = 0.0)
    let downside_variance = daily_net_returns
        .iter()
        .map(|&r| if r < 0.0 { r.powi(2) } else { 0.0 })
        .sum::<f64>()
        / n_days;
    let downside_std_dev = downside_variance.sqrt();
    let sortino_ratio = if downside_std_dev > 1e-9 {
        (mean_ret / downside_std_dev) * 252.0_f64.sqrt()
    } else if mean_ret > 0.0 {
        99.99
    } else {
        0.0
    };

    // Max Drawdown
    let mut peak = initial_capital;
    let mut max_drawdown = 0.0;
    for &eq in &equity_curve {
        if eq > peak {
            peak = eq;
        } else if peak > 0.0 {
            let dd = (peak - eq) / peak;
            if dd > max_drawdown {
                max_drawdown = dd;
            }
        }
    }

    // Win Rate
    let num_trades = if total_trades_count > 0 {
        total_trades_count
    } else {
        closed_trade_returns.len()
    };
    let win_rate = if !closed_trade_returns.is_empty() {
        let win_count = closed_trade_returns.iter().filter(|&&r| r > 0.0).count();
        (win_count as f64 / closed_trade_returns.len() as f64) * 100.0
    } else if !daily_net_returns.is_empty() {
        let win_count = daily_net_returns.iter().filter(|&&r| r > 0.0).count();
        (win_count as f64 / daily_net_returns.len() as f64) * 100.0
    } else {
        0.0
    };

    // Profit Factor (Gross Profits / Gross Losses)
    let gross_profit: f64 = daily_net_returns.iter().filter(|&&r| r > 0.0).sum();
    let gross_loss: f64 = daily_net_returns
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

    // Benchmark performance & Alpha
    let benchmark_total_return = (benchmark_equity - initial_capital) / initial_capital;
    let alpha = total_return - benchmark_total_return;

    let primary_ticker = if tickers.len() == 1 {
        tickers[0].clone()
    } else {
        tickers.join(", ")
    };

    BacktestResponse {
        ticker: primary_ticker,
        tickers: tickers.to_vec(),
        weights: weights.to_vec(),
        benchmark_ticker: benchmark_ticker.to_string(),
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        total_return: (total_return * 10_000.0).round() / 10_000.0,
        annualized_return: (annualized_return * 10_000.0).round() / 10_000.0,
        sharpe_ratio: (sharpe_ratio * 100.0).round() / 100.0,
        sortino_ratio: (sortino_ratio * 100.0).round() / 100.0,
        max_drawdown: (max_drawdown * 10_000.0).round() / 10_000.0,
        num_trades,
        win_rate: (win_rate * 10.0).round() / 10.0,
        profit_factor: (profit_factor * 100.0).round() / 100.0,
        transaction_cost_bps,
        equity_curve,
        equity_points,
        benchmark_curve,
        benchmark_total_return: (benchmark_total_return * 10_000.0).round() / 10_000.0,
        alpha: (alpha * 10_000.0).round() / 10_000.0,
        message: message.to_string(),
    }
}

/// Legacy single-ticker backward compatibility wrapper for existing tests.
pub fn run_simulation(
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    events: &[(f64, i64)],
    long_threshold: f64,
    short_threshold: f64,
    holding_days: i64,
    initial_capital: f64,
    message: &str,
) -> BacktestResponse {
    let mut map = HashMap::new();
    map.insert(ticker.to_string(), events.to_vec());
    let bench_events = generate_mock_backtest_events("SPY", start_date, end_date);
    let empty_prices = HashMap::new();
    run_portfolio_simulation(
        &[ticker.to_string()],
        &[1.0],
        "SPY",
        start_date,
        end_date,
        &map,
        &bench_events,
        &empty_prices,
        long_threshold,
        short_threshold,
        holding_days,
        initial_capital,
        DEFAULT_TRANSACTION_COST_BPS,
        message,
    )
}

/// Generates synthetic sentiment events for deterministic mock testing.
pub fn generate_mock_backtest_events(
    _ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<(f64, i64)> {
    let mut events = Vec::new();
    let mut curr = start_date;
    let mut day_idx = 0;

    while curr <= end_date {
        let dt = curr.and_hms_opt(14, 30, 0).unwrap().and_utc();
        let ts_us = dt.timestamp_micros();

        // Alternating bull/bear market cycle signals
        let score = (day_idx as f64 * 0.15).sin() * 0.75;
        events.push((score, ts_us));

        curr += Duration::days(1);
        day_idx += 1;
    }

    events
}

/// Generates synthetic daily stock prices for deterministic mock testing.
pub fn generate_mock_stock_prices(
    ticker: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> HashMap<NaiveDate, f64> {
    let mut map = HashMap::new();
    let mut curr = start_date;
    let mut day_idx = 0usize;
    let clean = ticker.trim().to_uppercase();
    let hash_val = clean.bytes().map(|b| b as usize).sum::<usize>();

    let mut price = match clean.as_str() {
        "AAPL" => 224.50,
        "NVDA" => 125.00,
        "MSFT" => 415.00,
        "GOOGL" | "GOOG" => 165.00,
        "AMZN" => 185.00,
        "META" => 510.00,
        "TSLA" => 210.00,
        "SPY" => 550.00,
        _ => 100.0 + ((hash_val % 300) as f64),
    };

    while curr <= end_date {
        let phase = ((hash_val + day_idx * 13) as f64) * 0.09;
        let daily_change = (phase.sin() * 0.015) + 0.0004;
        price = (price * (1.0 + daily_change)).max(1.0);
        let rounded = (price * 100.0).round() / 100.0;
        map.insert(curr, rounded);
        curr += Duration::days(1);
        day_idx += 1;
    }

    map
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_single_asset_simulation_metrics() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 30).unwrap();

        let events = generate_mock_backtest_events("AAPL", start, end);
        let resp = run_simulation(
            "AAPL",
            start,
            end,
            &events,
            0.2,
            -0.2,
            5,
            1_000_000.0,
            "Test single asset run",
        );

        assert_eq!(resp.ticker, "AAPL");
        assert_eq!(resp.start_date, "2025-01-01");
        assert_eq!(resp.end_date, "2025-01-30");
        assert!(resp.equity_curve.len() > 20);
        assert_eq!(resp.equity_points.len(), resp.equity_curve.len());
        assert!(resp.num_trades > 0);
        assert!(resp.max_drawdown >= 0.0);
        assert!(resp.max_drawdown <= 1.0);
        assert!(resp.win_rate >= 0.0 && resp.win_rate <= 100.0);
        assert!(resp.sortino_ratio >= 0.0);
        assert!(resp.profit_factor >= 0.0);
        assert_eq!(resp.benchmark_ticker, "SPY");
        assert_eq!(resp.benchmark_curve.len(), resp.equity_curve.len());
    }

    #[test]
    fn test_multi_asset_portfolio_simulation() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

        let tickers = vec!["AAPL".to_string(), "NVDA".to_string(), "MSFT".to_string()];
        let weights = vec![0.5, 0.3, 0.2];

        let mut ticker_events = HashMap::new();
        for t in &tickers {
            ticker_events.insert(t.clone(), generate_mock_backtest_events(t, start, end));
        }
        let bench_events = generate_mock_backtest_events("SPY", start, end);
        let empty_prices = HashMap::new();

        let resp = run_portfolio_simulation(
            &tickers,
            &weights,
            "SPY",
            start,
            end,
            &ticker_events,
            &bench_events,
            &empty_prices,
            0.2,
            -0.2,
            5,
            1_000_000.0,
            10.0, // 10 bps
            "Multi-asset portfolio test",
        );

        assert_eq!(resp.tickers, tickers);
        assert_eq!(resp.weights, weights);
        assert_eq!(resp.transaction_cost_bps, 10.0);
        assert!(resp.equity_curve.len() >= 90);
        assert!(resp.equity_points.len() == resp.equity_curve.len());
        assert!(resp.sharpe_ratio != 0.0);
        assert!(resp.sortino_ratio != 0.0);
        assert!(resp.profit_factor > 0.0);
        assert!(resp.benchmark_total_return != 0.0);
    }

    #[test]
    fn test_actual_price_based_backtest_simulation() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 3, 31).unwrap();

        let tickers = vec!["AAPL".to_string(), "NVDA".to_string()];
        let weights = vec![0.6, 0.4];

        let mut ticker_events = HashMap::new();
        let mut ticker_prices = HashMap::new();

        for t in &tickers {
            ticker_events.insert(t.clone(), generate_mock_backtest_events(t, start, end));
            ticker_prices.insert(t.clone(), generate_mock_stock_prices(t, start, end));
        }
        let bench_events = generate_mock_backtest_events("SPY", start, end);
        ticker_prices.insert(
            "SPY".to_string(),
            generate_mock_stock_prices("SPY", start, end),
        );

        let resp = run_portfolio_simulation(
            &tickers,
            &weights,
            "SPY",
            start,
            end,
            &ticker_events,
            &bench_events,
            &ticker_prices,
            0.15,
            -0.15,
            3,
            1_000_000.0,
            5.0,
            "Point-in-time multi-asset backtest executed using actual OHLCV stock price returns with trading costs",
        );

        assert_eq!(resp.tickers.len(), 2);
        assert!(resp.total_return != 0.0);
        assert!(resp.annualized_return != 0.0);
        assert!(resp.sharpe_ratio != 0.0);
        assert!(resp.equity_curve.len() >= 90);
        assert!(resp.benchmark_total_return != 0.0);
        assert_eq!(
            resp.message,
            "Point-in-time multi-asset backtest executed using actual OHLCV stock price returns with trading costs"
        );
    }
}
