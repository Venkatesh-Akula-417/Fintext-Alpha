import http from 'k6/http';
import { check, sleep, group } from 'k6';
import { Trend, Rate, Counter } from 'k6/metrics';

// Custom metric tracking
export const healthLatency = new Trend('health_duration');
export const sentimentLatency = new Trend('sentiment_duration');
export const batchSentimentLatency = new Trend('batch_sentiment_duration');
export const marketBreadthLatency = new Trend('market_breadth_duration');
export const backtestLatency = new Trend('backtest_duration');
export const successfulRequests = new Counter('successful_requests');

// ─────────────────────────────────────────────────────────────────────────────
// 1. Workload Profile & Quality of Service (SLA) Thresholds
// ─────────────────────────────────────────────────────────────────────────────
export const options = {
  // Ramp-up profile: 0 -> 100 VUs over 1m, 100 VUs for 3m, 100 -> 0 over 1m
  stages: [
    { duration: '1m', target: 100 }, // 1 minute ramp up to 100 concurrent virtual users
    { duration: '3m', target: 100 }, // 3 minutes steady sustained peak load at 100 VUs
    { duration: '1m', target: 0 },   // 1 minute ramp down to 0 VUs
  ],

  // Strict Service Level Agreements (SLAs)
  thresholds: {
    // 95th percentile latency must remain strictly under 500ms, 99th percentile under 1000ms
    'http_req_duration': ['p(95)<500', 'p(99)<1000'],
    // Total HTTP request failure rate must be strictly under 1% (< 0.01)
    'http_req_failed': ['rate<0.01'],
  },
};

// Target environment configuration
const BASE_URL = (__ENV.BASE_URL || 'http://127.0.0.1:8000').replace(/\/+$/, '');
const TEST_JWT_TOKEN = __ENV.TEST_JWT_TOKEN || '';

// Common HTTP request headers
const authHeaders = {
  'Authorization': `Bearer ${TEST_JWT_TOKEN}`,
  'Content-Type': 'application/json',
  'Accept': 'application/json',
};

const jsonHeaders = {
  'Content-Type': 'application/json',
  'Accept': 'application/json',
};

// ─────────────────────────────────────────────────────────────────────────────
// 2. Main Virtual User (VU) Execution Scenario
// ─────────────────────────────────────────────────────────────────────────────
export default function () {
  // Execute heavy POST /backtest simulation once every 10 iterations
  const isBacktestIteration = (__ITER % 10 === 0);

  if (isBacktestIteration) {
    group('POST /backtest - Multi-Asset Portfolio Alpha Strategy Simulation', function () {
      const payload = JSON.stringify({
        tickers: ['AAPL', 'MSFT', 'NVDA'],
        start_date: '2025-01-01',
        end_date: '2025-03-31',
        long_threshold: 0.2,
        short_threshold: -0.2,
        holding_days: 5,
        initial_capital: 1000000.0,
      });

      const res = http.post(`${BASE_URL}/backtest`, payload, { headers: authHeaders });
      backtestLatency.add(res.timings.duration);

      const passed = check(res, {
        'backtest status is 200': (r) => r.status === 200,
        'backtest total_return present': (r) => r.status === 200 && r.json('total_return') !== undefined,
        'backtest sharpe_ratio present': (r) => r.status === 200 && r.json('sharpe_ratio') !== undefined,
        'backtest equity_curve present': (r) => r.status === 200 && Array.isArray(r.json('equity_curve')),
      });

      if (passed) successfulRequests.add(1);
    });
  } else {
    // Weighted endpoint traffic distribution:
    // 40% Health, 30% Single Sentiment, 20% Batch Sentiment, 10% Market Breadth
    const rand = Math.random();

    if (rand < 0.40) {
      // 40% Traffic: Public Health Check Probe
      group('GET /health - System Liveness & Health Probe', function () {
        const res = http.get(`${BASE_URL}/health`, { headers: jsonHeaders });
        healthLatency.add(res.timings.duration);

        const passed = check(res, {
          'health status is 200': (r) => r.status === 200,
          'health status ok': (r) => r.status === 200 && r.json('status') === 'ok',
        });

        if (passed) successfulRequests.add(1);
      });
    } else if (rand < 0.70) {
      // 30% Traffic: Single-Ticker Real-Time Sentiment
      group('GET /sentiment - Single Ticker Point-in-Time Sentiment', function () {
        const tickers = ['AAPL', 'MSFT', 'NVDA', 'AMZN', 'GOOGL'];
        const ticker = tickers[Math.floor(Math.random() * tickers.length)];

        const res = http.get(`${BASE_URL}/sentiment?ticker=${ticker}`, { headers: authHeaders });
        sentimentLatency.add(res.timings.duration);

        const passed = check(res, {
          'sentiment status is 200': (r) => r.status === 200,
          'sentiment score present': (r) => r.status === 200 && r.json('sentiment_score') !== undefined,
          'sentiment label present': (r) => r.status === 200 && r.json('sentiment_label') !== undefined,
        });

        if (passed) successfulRequests.add(1);
      });
    } else if (rand < 0.90) {
      // 20% Traffic: Multi-Ticker Batch Sentiment
      group('GET /sentiment/batch - Multi-Ticker Batch Sentiment', function () {
        const res = http.get(`${BASE_URL}/sentiment/batch?tickers=AAPL,MSFT,NVDA`, { headers: authHeaders });
        batchSentimentLatency.add(res.timings.duration);

        const passed = check(res, {
          'batch sentiment status is 200': (r) => r.status === 200,
          'batch sentiment results present': (r) => r.status === 200 && Array.isArray(r.json('results')),
        });

        if (passed) successfulRequests.add(1);
      });
    } else {
      // 10% Traffic: Market Breadth Advance/Decline Time-Series
      group('GET /market/breadth - Advance/Decline Market Breadth', function () {
        const res = http.get(
          `${BASE_URL}/market/breadth?start_date=2025-01-01&end_date=2025-01-31`,
          { headers: authHeaders }
        );
        marketBreadthLatency.add(res.timings.duration);

        const passed = check(res, {
          'market breadth status is 200': (r) => r.status === 200,
          'market breadth points present': (r) => r.status === 200 && (r.json('breadth') !== undefined || r.json('points') !== undefined),
        });

        if (passed) successfulRequests.add(1);
      });
    }
  }

  // User think time / pacing between request iterations
  sleep(1.0);
}
