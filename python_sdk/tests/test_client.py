"""
Unit tests for synchronous FinTextClient using httpx.MockTransport.
"""

import json
import httpx
import pytest

from fintext import (
    AcousticFeatures,
    ApiKeyItem,
    AudioSentiment,
    AudioTranscriptionResponse,
    BacktestRequest,
    CheckoutResponse,
    CreateApiKeyResponse,
    DeleteTranscriptResponse,
    FinTextAPIError,
    FinTextAuthError,
    FinTextClient,
    FinTextRateLimitError,
    FinTextValidationError,
    ListApiKeysResponse,
    RotateApiKeyResponse,
    SearchResponse,
    SearchResultItem,
    EarningsSurpriseItem,
    EarningsSurpriseResponse,
    HealthResponse,
    InsiderTradeItem,
    InsiderTradingResponse,
    MARumorItem,
    MARumorsResponse,
    OptionsIvResponse,
    OptionsVolSurfaceResponse,
    PortalResponse,
    PutCallRatioResponse,
    RegulatoryFilingItem,
    RegulatoryFilingsResponse,
    ReturnCorrelationItem,
    ReturnCorrelationResponse,
    SentimentAnomaliesResponse,
    SentimentAnomalyItem,
    SentimentDisagreementResponse,
    SentimentResponse,
    SourceBreakdown,
    SpilloverResponse,
    SubscriptionResponse,
    TranscriptListResponse,
    TranscriptMetadata,
    TranscriptResponse,
    VolSurfacePoint,
    KafkaTopicInfo,
    KafkaTopicsResponse,
    KafkaCredentials,
    RevokeKafkaCredentialsResponse,
    RetentionPolicy,
    CreateRetentionPolicyRequest,
    RetentionPoliciesResponse,
    DeleteRetentionPolicyResponse,
    FactorExposureItem,
    OLSStatistics,
    FactorExposureResponse,
    ESGDimensionScore,
    ESGDimensions,
    ESGScoresResponse,
    BankruptcyComponents,
    BankruptcyRiskResponse,
    FXSentimentArticle,
    FXSentimentResponse,
    FXSentimentSummary,
    CommoditySentimentArticle,
    CommoditySentimentResponse,
    CommoditySentimentSummary,
    CryptoSentimentArticle,
    CryptoSentimentResponse,
    CryptoSentimentSummary,
    MicrostructurePoint,
    MicrostructureResponse,
    MarketBreadthPoint,
    MarketBreadthResponse,
    CreatePollingWebhookRequest,
    DeletePollingWebhookResponse,
    PollingWebhook,
    PollingWebhooksResponse,
    ChatAlertSubscription,
    CreateChatAlertRequest,
    ChatAlertsResponse,
    DeleteChatAlertResponse,
    CreditSentimentResponse,
    BackfillSentimentRequest,
    BackfillSentimentResponse,
    PortfolioConstraints,
    PortfolioOptimizeRequest,
    PortfolioOptimizeResponse,
    PortfolioWeight,
    PortfolioFactorExposureRequest,
    PortfolioFactorExposureResponse,
    RetrainingJob,
    CreateRetrainingJobRequest,
    RetrainingJobResponse,
    ListRetrainingJobsResponse,
    FIXOrderRequest,
    FIXCancelRequest,
    FIXOrderResponse,
    FixOrderItem,
    FIXOrdersListResponse,
    DLQEventItem,
    DLQEventDetail,
    DLQEventsListResponse,
    ReprocessDLQResponse,
    PurgeDLQResponse,
    SLAStatusResponse,
    SandboxStatusResponse,
    SandboxStatus,
    DataProvenanceResponse,
    DataProvenance,
    ProcessingStep,
    DataProvenanceItem,
    AnomalyScanResponse,
    SentimentAnomalyAlert,
    ProviderHealthItem,
    ProviderHealthResponse,
    ProviderHealth,
)


def create_mock_transport() -> httpx.MockTransport:
    def handler(request: httpx.Request) -> httpx.Response:
        url_path = request.url.path
        method = request.method

        # 1. GET /health
        if url_path == "/health" and method == "GET":
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "version": "2.0.0-institutional",
                    "timestamp_us": 1787940389739799,
                },
            )

        # 2. POST /auth/token
        if url_path == "/auth/token" and method == "POST":
            admin_header = request.headers.get("X-Admin-Token")
            body = json.loads(request.content)
            user_id = body.get("user_id", "")

            if not user_id:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Missing user_id"})

            if admin_header == "valid_admin_token":
                return httpx.Response(
                    200,
                    json={
                        "token": f"jwt_mock_token_for_{user_id}",
                        "token_type": "Bearer",
                        "expires_in": 3600,
                        "user_id": user_id,
                        "role": "institutional",
                    },
                )
            return httpx.Response(401, json={"error": "Unauthorized", "message": "Invalid X-Admin-Token"})

        # 3. GET /sentiment
        if url_path == "/sentiment" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            if "rate_limited_token" in auth:
                return httpx.Response(
                    429,
                    json={"error": "Too Many Requests", "message": "Rate limit exceeded. Quota: 100 per 60s."},
                    headers={
                        "X-RateLimit-Limit": "100",
                        "X-RateLimit-Remaining": "0",
                        "X-RateLimit-Reset": "15",
                        "Retry-After": "15",
                    },
                )

            ticker = request.url.params.get("ticker", "")
            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "date": "2026-08-25",
                    "sentiment_score": 0.45,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.88,
                    "probabilities": {
                        "positive": 0.88,
                        "neutral": 0.08,
                        "negative": 0.04,
                    },
                    "signal_available_ts_us": 1787940389786186,
                    "data_quality_score": 0.88,
                    "message": "Point-in-time sentiment retrieved",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 3b. GET /sentiment/history
        if url_path == "/sentiment/history" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-01-10")
            limit = int(request.url.params.get("limit", 100))
            offset = int(request.url.params.get("offset", 0))
            sort = request.url.params.get("sort", "asc")

            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "start_date": start_date,
                    "end_date": end_date,
                    "count": 2,
                    "total": 10,
                    "limit": limit,
                    "offset": offset,
                    "sort": sort,
                    "records": [
                        {
                            "published_utc": "2025-01-01T14:30:00.000000Z",
                            "ticker": ticker,
                            "source": "Institutional Wire",
                            "title": f"{ticker} Market Sentiment Analysis",
                            "sentiment_score": 0.25,
                            "vpin": 0.55,
                            "gamma_exposure": 0.0,
                            "data_quality_score": 0.88,
                        },
                        {
                            "published_utc": "2025-01-02T14:30:00.000000Z",
                            "ticker": ticker,
                            "source": "Institutional Wire",
                            "title": f"{ticker} Market Sentiment Analysis",
                            "sentiment_score": 0.35,
                            "vpin": 0.54,
                            "gamma_exposure": 59940.0,
                            "data_quality_score": 0.88,
                        },
                    ],
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # GET /sentiment/feed
        if url_path == "/sentiment/feed" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            sector = request.url.params.get("sector")
            start_date = request.url.params.get("start_date", "2026-08-28T00:00:00Z")
            end_date = request.url.params.get("end_date", "2026-08-29T00:00:00Z")
            min_confidence = float(request.url.params.get("min_confidence", 0.0))
            min_quality = float(request.url.params.get("min_quality", 0.0))
            cursor = request.url.params.get("cursor")
            offset_param = request.url.params.get("offset")
            offset = int(offset_param) if offset_param is not None else 0
            limit = int(request.url.params.get("limit", 100))
            sort = request.url.params.get("sort", "desc")

            if sector == "InvalidSector":
                return httpx.Response(404, json={"error": "Not Found", "message": "Unknown sector 'InvalidSector'"})

            records = [
                {
                    "published_utc": "2026-08-29T14:30:00.000000Z",
                    "ticker": "AAPL",
                    "source": "Institutional Wire",
                    "title": "AAPL Market Sentiment and Flow Analysis",
                    "sentiment_score": 0.45,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.88,
                    "data_quality_score": 0.92,
                    "vpin": 0.42,
                    "gamma_exposure": 150000.0,
                },
                {
                    "published_utc": "2026-08-29T14:15:00.000000Z",
                    "ticker": "NVDA",
                    "source": "Bloomberg",
                    "title": "NVDA Next-Generation AI Accelerator Expansion",
                    "sentiment_score": 0.72,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.94,
                    "data_quality_score": 0.95,
                    "vpin": 0.38,
                    "gamma_exposure": 280000.0,
                },
            ]

            return httpx.Response(
                200,
                json={
                    "count": len(records),
                    "total": 50,
                    "limit": limit,
                    "offset": offset,
                    "cursor": cursor,
                    "next_cursor": "2026-08-29T14:15:00.000000Z" if len(records) > 0 else None,
                    "sort": sort,
                    "sector": sector,
                    "start_time": start_date,
                    "end_time": end_date,
                    "min_confidence": min_confidence,
                    "min_quality": min_quality,
                    "records": records,
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # GET /sentiment/anomalies
        if url_path == "/sentiment/anomalies" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            sector = request.url.params.get("sector")
            if sector == "InvalidSector":
                return httpx.Response(404, json={"error": "Not Found", "message": "Unknown sector 'InvalidSector'"})

            lookback_days = int(request.url.params.get("lookback_days", 30))
            zscore_threshold = float(request.url.params.get("zscore_threshold", 2.0))
            min_records = int(request.url.params.get("min_records", 20))
            limit = int(request.url.params.get("limit", 20))

            items = [
                {
                    "ticker": "AAPL",
                    "latest_score": 0.85,
                    "mean_score": 0.22,
                    "stddev": 0.21,
                    "zscore": 3.0,
                    "direction": "bullish",
                    "latest_timestamp": "2026-08-29T14:30:00.000000Z",
                    "record_count": 45,
                },
                {
                    "ticker": "MSFT",
                    "latest_score": -0.65,
                    "mean_score": 0.15,
                    "stddev": 0.25,
                    "zscore": -3.2,
                    "direction": "bearish",
                    "latest_timestamp": "2026-08-29T14:25:00.000000Z",
                    "record_count": 42,
                },
            ]

            return httpx.Response(
                200,
                json={
                    "count": len(items),
                    "total_anomalies_detected": 2,
                    "lookback_days": lookback_days,
                    "zscore_threshold": zscore_threshold,
                    "min_records": min_records,
                    "sector": sector,
                    "scanned_tickers": 12,
                    "items": items,
                    "generated_at": "2026-08-29T14:30:00.000000Z",
                    "message": "Sentiment anomaly detection completed: 2 anomalies detected across 12 tickers",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # POST /audio/transcribe
        if url_path == "/audio/transcribe" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            content_type = request.headers.get("Content-Type", "")
            if "multipart/form-data" not in content_type:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Expected multipart/form-data"})

            body_content = request.content
            if b"invalid.txt" in body_content or b"document.pdf" in body_content:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Unsupported audio format. Allowed: WAV, MP3, FLAC, M4A, AAC, OGG"})

            stored_id = "t-12345-uuid" if (b"name=\"store\"\r\n\r\ntrue" in body_content or request.url.params.get("store") == "true") else None

            return httpx.Response(
                200,
                json={
                    "transcription": "Apple Inc. reported record quarterly revenue of $94.9 billion, up 6 percent year over year. Operating cash flow reached $26.8 billion.",
                    "duration_seconds": 120.5,
                    "acoustic_features": {
                        "pitch_mean_hz": 120.3,
                        "energy_rms": 0.05,
                        "pause_ratio": 0.15,
                    },
                    "sentiment": {
                        "score": 0.725,
                        "label": "BULLISH",
                        "confidence": 0.892,
                    },
                    "word_count": 24,
                    "language": "en",
                    "transcript_id": stored_id,
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # GET /market/regime
        if url_path == "/market/regime" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            lookback_days = int(request.url.params.get("lookback_days", 5))
            if lookback_days < 1 or lookback_days > 30:
                return httpx.Response(400, json={"error": "Bad Request", "message": "lookback_days must be between 1 and 30"})

            min_data_points = int(request.url.params.get("min_data_points", 50))
            if min_data_points < 1 or min_data_points > 1000:
                return httpx.Response(400, json={"error": "Bad Request", "message": "min_data_points must be between 1 and 1000"})

            return httpx.Response(
                200,
                json={
                    "regime": "Bullish",
                    "confidence": 0.85,
                    "market_sentiment": 0.24,
                    "breadth": 0.68,
                    "avg_spillover_corr": 0.42,
                    "volatility_proxy": 0.18,
                    "lookback_days": lookback_days,
                    "generated_at": "2026-08-30T04:45:00.000000Z",
                    "components": {
                        "sector_sentiments": {
                            "Technology": 0.32,
                            "Financials": 0.18,
                            "Healthcare": 0.10,
                            "Energy": -0.05,
                        },
                        "total_data_points": 120,
                        "positive_sentiment_ratio": 0.68,
                        "bullish_tickers_count": 14,
                        "bearish_tickers_count": 5,
                        "neutral_tickers_count": 1,
                    },
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "99",
                    "X-RateLimit-Reset": "60",
                },
            )

        # GET /market/correlation
        if url_path == "/market/correlation" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            tickers_param = request.url.params.get("tickers", "").strip()
            if not tickers_param:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Parameter 'tickers' must not be empty"})

            tickers_list = [t.strip().upper() for t in tickers_param.split(",") if t.strip()]
            start_date = request.url.params.get("start_date", "")
            end_date = request.url.params.get("end_date", "")
            min_periods = int(request.url.params.get("min_periods", 20))
            include_self = request.url.params.get("include_self") == "true"

            if min_periods < 10 or min_periods > 1000:
                return httpx.Response(400, json={"error": "Bad Request", "message": "min_periods must be between 10 and 1000"})

            matrix = []
            for i in range(len(tickers_list)):
                t_a = tickers_list[i]
                if include_self:
                    matrix.append({
                        "ticker_a": t_a,
                        "ticker_b": t_a,
                        "correlation": 1.0,
                        "periods": 60,
                    })
                for j in range(i + 1, len(tickers_list)):
                    t_b = tickers_list[j]
                    matrix.append({
                        "ticker_a": t_a,
                        "ticker_b": t_b,
                        "correlation": 0.72,
                        "periods": 60,
                    })

            return httpx.Response(
                200,
                json={
                    "tickers": tickers_list,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_periods": min_periods,
                    "matrix": matrix,
                    "generated_at": "2026-08-30T10:30:00.000000Z",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "99",
                    "X-RateLimit-Reset": "60",
                },
            )

        # GET /options/put-call-ratio
        if url_path == "/options/put-call-ratio" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            start_date = request.url.params.get("start_date", "").strip()
            end_date = request.url.params.get("end_date", "").strip()
            if not start_date or not end_date:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Dates must not be empty"})

            ticker = request.url.params.get("ticker")
            ratio_type = request.url.params.get("ratio_type", "volume").lower()
            granularity = request.url.params.get("granularity", "daily").lower()

            if ratio_type not in ("volume", "open_interest"):
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid ratio_type"})
            if granularity not in ("daily", "total"):
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid granularity"})

            if granularity == "total":
                return httpx.Response(
                    200,
                    json={
                        "ticker": ticker.upper() if ticker else None,
                        "start_date": start_date,
                        "end_date": end_date,
                        "ratio_type": ratio_type,
                        "granularity": "total",
                        "total_put_volume": 450000,
                        "total_call_volume": 900000,
                        "total_put_open_interest": 4500000,
                        "total_call_open_interest": 9000000,
                        "total_ratio": 0.50,
                        "generated_at": "2026-08-30T10:00:00Z",
                    },
                    headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
                )
            else:
                return httpx.Response(
                    200,
                    json={
                        "ticker": ticker.upper() if ticker else None,
                        "start_date": start_date,
                        "end_date": end_date,
                        "ratio_type": ratio_type,
                        "granularity": "daily",
                        "points": [
                            {"date": "2025-01-02", "put_volume": 1200, "call_volume": 3000, "ratio": 0.40},
                            {"date": "2025-01-03", "put_volume": 1500, "call_volume": 2500, "ratio": 0.60},
                        ],
                        "average_ratio": 0.50,
                        "generated_at": "2026-08-30T10:00:00Z",
                    },
                    headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
                )

        # GET /events/earnings-surprise
        if url_path == "/events/earnings-surprise" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-03-31")
            min_shift = float(request.url.params.get("min_sentiment_shift", "0.15"))
            pre_days = int(request.url.params.get("pre_days", "5"))
            post_days = int(request.url.params.get("post_days", "5"))
            limit = int(request.url.params.get("limit", "20"))

            surprises = [
                {
                    "ticker": ticker.upper() if ticker else "AAPL",
                    "earnings_date": "2025-01-30",
                    "pre_avg_sentiment": 0.22,
                    "post_avg_sentiment": 0.58,
                    "surprise_score": 0.36,
                    "direction": "positive",
                    "pre_record_count": 5,
                    "post_record_count": 5,
                },
                {
                    "ticker": "NVDA" if not ticker else ticker.upper(),
                    "earnings_date": "2025-02-20",
                    "pre_avg_sentiment": 0.40,
                    "post_avg_sentiment": 0.15,
                    "surprise_score": -0.25,
                    "direction": "negative",
                    "pre_record_count": 5,
                    "post_record_count": 5,
                },
            ]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper() if ticker else None,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_sentiment_shift": min_shift,
                    "pre_days": pre_days,
                    "post_days": post_days,
                    "count": len(surprises),
                    "surprises": surprises,
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /events/insider-trading
        if url_path == "/events/insider-trading" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker")
            t_type = request.url.params.get("transaction_type", "all")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-03-31")
            min_shares = int(request.url.params.get("min_shares", "0"))
            min_score = float(request.url.params.get("min_signal_score", "0.0"))
            limit = int(request.url.params.get("limit", "20"))

            trades = [
                {
                    "ticker": ticker.upper() if ticker else "AAPL",
                    "insider_name": "Tim Cook",
                    "insider_role": "CEO",
                    "transaction_type": "purchase",
                    "shares": 25000,
                    "price": 225.0,
                    "value": 5625000.0,
                    "filing_date": "2025-02-15",
                    "signal_score": 0.9645,
                    "source": "SEC Form 4",
                },
                {
                    "ticker": "NVDA" if not ticker else ticker.upper(),
                    "insider_name": "Colette Kress",
                    "insider_role": "CFO",
                    "transaction_type": "sale",
                    "shares": 15000,
                    "price": 128.0,
                    "value": 1920000.0,
                    "filing_date": "2025-02-28",
                    "signal_score": -0.8975,
                    "source": "SEC Form 4",
                },
            ]

            filtered_trades = [
                t for t in trades
                if (t_type == "all" or t["transaction_type"].lower() == t_type.lower())
                and t["shares"] >= min_shares
                and abs(t["signal_score"]) >= min_score
            ][:limit]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper() if ticker else None,
                    "transaction_type": t_type,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_shares": min_shares,
                    "min_signal_score": min_score,
                    "count": len(filtered_trades),
                    "trades": filtered_trades,
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /sentiment/disagreement
        if url_path == "/sentiment/disagreement" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-03-31")
            source_filter = request.url.params.get("source")
            min_records = int(request.url.params.get("min_records", "10"))
            agg = request.url.params.get("aggregation", "stddev")

            breakdown = [
                {"source": "SEC EDGAR", "mean": 0.15, "count": 40},
                {"source": "Finnhub", "mean": 0.08, "count": 60},
                {"source": "Bloomberg", "mean": 0.22, "count": 50},
            ]

            if source_filter:
                breakdown = [b for b in breakdown if source_filter.lower() in b["source"].lower()]

            total_records = sum(b["count"] for b in breakdown)

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper(),
                    "start_date": start_date,
                    "end_date": end_date,
                    "aggregation": agg,
                    "disagreement_index": 0.1825 if agg == "stddev" else (0.2450 if agg == "iqr" else 0.1650),
                    "mean_sentiment": 0.1450,
                    "median_sentiment": 0.1200,
                    "record_count": total_records,
                    "source_count": len(breakdown),
                    "sources_breakdown": breakdown,
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /options/vol-surface
        if url_path == "/options/vol-surface" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            strike_count = int(request.url.params.get("strike_count", "9"))
            spot = 225.50

            strikes = [round(spot * (0.8 + 0.4 * i / (strike_count - 1)), 2) for i in range(strike_count)]
            expirations = ["2025-04-18", "2025-05-16", "2025-06-20"]
            surface = [
                {"expiration": exp, "ivs": [0.25 + 0.02 * i for i in range(strike_count)]}
                for exp in expirations
            ]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper(),
                    "spot": spot,
                    "generated_at": "2026-08-30T10:00:00Z",
                    "strikes": strikes,
                    "expirations": expirations,
                    "surface": surface,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /events/ma-rumors
        if url_path == "/events/ma-rumors" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker")
            min_score = float(request.url.params.get("min_rumor_score", "0.5"))
            lookback = int(request.url.params.get("lookback_days", "7"))

            items = [
                {
                    "ticker": "PYPL",
                    "rumor_score": 0.85,
                    "sentiment_zscore": 3.10,
                    "keyword_hits": 4,
                    "recent_8k_ma_flag": True,
                    "supply_chain_related_tickers": ["V", "MA", "EBAY"],
                    "insider_net_buying": True,
                    "latest_news_title": "Activist investor builds stake, pushing board to evaluate takeover and buyout bids",
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                {
                    "ticker": "AMD",
                    "rumor_score": 0.78,
                    "sentiment_zscore": 2.95,
                    "keyword_hits": 4,
                    "recent_8k_ma_flag": True,
                    "supply_chain_related_tickers": ["TSM", "ASML", "DELL"],
                    "insider_net_buying": True,
                    "latest_news_title": "Industry sources report preliminary acquisition talks with specialized AI architecture firm",
                    "generated_at": "2026-08-30T10:00:00Z",
                }
            ]

            if ticker:
                items = [it for it in items if it["ticker"] == ticker.upper()]

            items = [it for it in items if it["rumor_score"] >= min_score]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper() if ticker else None,
                    "min_rumor_score": min_score,
                    "lookback_days": lookback,
                    "count": len(items),
                    "items": items,
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /events/filings
        if url_path == "/events/filings" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker")
            form_type = request.url.params.get("form_type")
            event_category = request.url.params.get("event_category")
            limit = int(request.url.params.get("limit", "20"))
            offset = int(request.url.params.get("offset", "0"))

            filings = [
                {
                    "ticker": "AAPL",
                    "form_type": "10-K",
                    "filing_date": "2025-10-25",
                    "accession_number": "0000320193-25-000100",
                    "event_category": "Annual Report",
                    "description": "Annual Report for Fiscal Year Ended September 30, 2025",
                    "url": "https://www.sec.gov/ix?doc=/Archives/edgar/data/320193/000032019325000100/aapl-20250930.htm",
                },
                {
                    "ticker": "NVDA",
                    "form_type": "8-K",
                    "filing_date": "2025-11-15",
                    "accession_number": "0001045810-25-000085",
                    "event_category": "M&A",
                    "description": "Item 1.01 Entry into a Material Definitive Purchase and Merger Agreement",
                    "url": "https://www.sec.gov/ix?doc=/Archives/edgar/data/1045810/000104581025000085/nvda-8k.htm",
                },
            ]

            if ticker:
                filings = [f for f in filings if f["ticker"] == ticker.upper()]
            if form_type:
                filings = [f for f in filings if f["form_type"] == form_type.upper()]
            if event_category:
                filings = [f for f in filings if f["event_category"].lower() == event_category.lower()]

            total_count = len(filings)
            paginated = filings[offset : offset + limit]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper() if ticker else None,
                    "form_type": form_type.upper() if form_type else None,
                    "event_category": event_category if event_category else None,
                    "start_date": "2025-08-01",
                    "end_date": "2025-11-30",
                    "count": len(paginated),
                    "total_count": total_count,
                    "offset": offset,
                    "limit": limit,
                    "filings": paginated,
                    "generated_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # /transcripts routes
        if url_path == "/transcripts" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content)
            ticker = body.get("ticker", "").upper()
            text = body.get("transcript_text", "")
            return httpx.Response(
                201,
                json={
                    "id": "t-12345-uuid",
                    "user_id": "test_quant_fund",
                    "ticker": ticker,
                    "quarter": body.get("quarter", 4),
                    "year": body.get("year", 2024),
                    "call_date": body.get("call_date", "2024-10-31"),
                    "transcript_text": text,
                    "source": body.get("source", "manual"),
                    "sentiment_score": body.get("sentiment_score", 0.725),
                    "sentiment_label": body.get("sentiment_label", "BULLISH"),
                    "confidence": body.get("confidence", 0.892),
                    "word_count": len(text.split()),
                    "created_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path == "/transcripts" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker_filter = request.url.params.get("ticker")
            items = [
                {
                    "id": "t-12345-uuid",
                    "user_id": "test_quant_fund",
                    "ticker": "AAPL",
                    "quarter": 4,
                    "year": 2024,
                    "call_date": "2024-10-31",
                    "source": "manual",
                    "sentiment_score": 0.725,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.892,
                    "word_count": 24,
                    "created_at": "2026-08-30T10:00:00Z",
                }
            ]
            if ticker_filter and ticker_filter.upper() != "AAPL":
                items = []

            return httpx.Response(
                200,
                json={
                    "total": len(items),
                    "count": len(items),
                    "limit": int(request.url.params.get("limit", 20)),
                    "offset": int(request.url.params.get("offset", 0)),
                    "items": items,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path.startswith("/transcripts/") and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            tid = url_path.split("/")[-1]
            if tid == "not-found":
                return httpx.Response(404, json={"error": "Not Found", "message": "Transcript not found"})

            return httpx.Response(
                200,
                json={
                    "id": tid,
                    "user_id": "test_quant_fund",
                    "ticker": "AAPL",
                    "quarter": 4,
                    "year": 2024,
                    "call_date": "2024-10-31",
                    "transcript_text": "Apple Inc. reported record quarterly revenue of $94.9 billion.",
                    "source": "manual",
                    "sentiment_score": 0.725,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.892,
                    "word_count": 9,
                    "created_at": "2026-08-30T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path.startswith("/transcripts/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            tid = url_path.split("/")[-1]
            if tid == "not-found":
                return httpx.Response(404, json={"error": "Not Found", "message": "Transcript not found"})

            return httpx.Response(
                200,
                json={
                    "id": tid,
                    "deleted": True,
                    "message": "Transcript successfully deleted",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        # GET /export/csv
        if url_path == "/export/csv" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            header_row = "published_utc,ticker,source,title,sentiment_score,vpin,gamma_exposure,data_quality_score\r\n"
            data_row = f"2025-01-01T14:30:00.000000Z,{ticker},Institutional Wire,{ticker} Analysis,0.2500,0.5500,0,0.8800\r\n"
            return httpx.Response(200, text=header_row + data_row, headers={"Content-Type": "text/csv"})

        # GET /export/parquet
        if url_path == "/export/parquet" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            start = request.url.params.get("start_date", "2025-01-01")
            end = request.url.params.get("end_date", "2025-01-10")
            dummy_parquet = b"PAR1\x00\x00\x00\x00\x00\x00\x00\x00PAR1"
            return httpx.Response(
                200,
                content=dummy_parquet,
                headers={
                    "Content-Type": "application/octet-stream",
                    "Content-Disposition": f'attachment; filename="sentiment_{ticker}_{start}_{end}.parquet"',
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "99",
                    "X-RateLimit-Reset": "60",
                },
            )

        # 3c. GET /sentiment/sector
        if url_path == "/sentiment/sector" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            sector = request.url.params.get("sector", "")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-03-31")
            aggregation = request.url.params.get("aggregation", "average")
            min_confidence = float(request.url.params.get("min_confidence", 0.0))

            if sector == "InvalidSector":
                return httpx.Response(404, json={"error": "Not Found", "message": "Unknown sector 'InvalidSector'"})

            return httpx.Response(
                200,
                json={
                    "sector": sector,
                    "start_date": start_date,
                    "end_date": end_date,
                    "aggregation": aggregation,
                    "min_confidence": min_confidence,
                    "value": 0.42,
                    "record_count": 150,
                    "tickers_included": 22,
                    "generated_at": "2026-08-29T12:00:00Z",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "96",
                    "X-RateLimit-Reset": "57",
                },
            )

        # 3d. GET /sentiment/batch
        if url_path == "/sentiment/batch" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            tickers_param = request.url.params.get("tickers")
            universe_id = request.url.params.get("universe_id")
            date_param = request.url.params.get("date", "LATEST")

            if tickers_param and universe_id:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Specify either tickers or universe_id, not both"})
            if not tickers_param and not universe_id:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Either tickers or universe_id must be provided"})

            if universe_id:
                if universe_id == "u-123":
                    ticker_list = ["AAPL", "MSFT"]
                else:
                    return httpx.Response(404, json={"error": "Not Found", "message": f"Universe '{universe_id}' not found"})
            else:
                ticker_list = [t.strip().upper() for t in tickers_param.split(",") if t.strip()]

            results = []
            for t in ticker_list:
                results.append({
                    "ticker": t,
                    "date": date_param,
                    "sentiment_score": 0.50,
                    "sentiment_label": "BULLISH",
                    "confidence": 0.85,
                    "probabilities": {
                        "positive": 0.85,
                        "neutral": 0.10,
                        "negative": 0.05,
                    },
                    "signal_available_ts_us": 1787940389786186,
                    "message": "Point-in-time sentiment signal retrieved",
                })

            return httpx.Response(
                200,
                json={
                    "count": len(results),
                    "results": results,
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "95",
                    "X-RateLimit-Reset": "56",
                },
            )

        # 3e. GET /symbols/map
        if url_path == "/symbols/map" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            identifier = request.url.params.get("identifier", "").strip().upper()
            input_type = request.url.params.get("input_type", "auto").strip().lower()
            output_type = request.url.params.get("output_type", "all").strip().lower()

            if not identifier:
                return httpx.Response(400, json={"error": "Bad Request", "message": "identifier cannot be empty"})

            db = {
                "AAPL": {"ticker": "AAPL", "figi": "BBG000B9XRY4", "cusip": "037833100", "isin": "US0378331005"},
                "US0378331005": {"ticker": "AAPL", "figi": "BBG000B9XRY4", "cusip": "037833100", "isin": "US0378331005"},
                "BBG000B9XRY4": {"ticker": "AAPL", "figi": "BBG000B9XRY4", "cusip": "037833100", "isin": "US0378331005"},
                "MSFT": {"ticker": "MSFT", "figi": "BBG000BPH459", "cusip": "594918104", "isin": "US5949181045"},
            }

            if identifier not in db:
                return httpx.Response(404, json={"error": "Not Found", "message": f"No security identifier mapping found for '{identifier}'"})

            full_ids = db[identifier]
            resolved_type = "ticker" if len(identifier) <= 5 else ("figi" if identifier.startswith("BBG") else "isin")
            if input_type != "auto":
                resolved_type = input_type

            if output_type == "all":
                result = full_ids
            else:
                result = {k: v for k, v in full_ids.items() if k == output_type}

            return httpx.Response(
                200,
                json={
                    "input_identifier": identifier,
                    "input_type": resolved_type,
                    "output_type": output_type,
                    "result": result,
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "94",
                    "X-RateLimit-Reset": "55",
                },
            )

        # 3f. /universes routes
        if url_path == "/universes" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content)
            return httpx.Response(
                201,
                json={
                    "id": "u-123",
                    "user_id": "test_quant_fund",
                    "name": body["name"],
                    "tickers": body["tickers"],
                    "created_at": "2026-08-29T12:00:00Z",
                    "updated_at": "2026-08-29T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path == "/universes" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "universes": [
                        {
                            "id": "u-123",
                            "user_id": "test_quant_fund",
                            "name": "Tech Watchlist",
                            "tickers": ["AAPL", "MSFT"],
                            "created_at": "2026-08-29T12:00:00Z",
                            "updated_at": "2026-08-29T12:00:00Z",
                        }
                    ],
                    "count": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path.startswith("/universes/") and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            uid = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "id": uid,
                    "user_id": "test_quant_fund",
                    "name": "Tech Watchlist",
                    "tickers": ["AAPL", "MSFT"],
                    "created_at": "2026-08-29T12:00:00Z",
                    "updated_at": "2026-08-29T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path.startswith("/universes/") and method == "PUT":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            uid = url_path.split("/")[-1]
            body = json.loads(request.content)
            return httpx.Response(
                200,
                json={
                    "id": uid,
                    "user_id": "test_quant_fund",
                    "name": body.get("name", "Updated Watchlist"),
                    "tickers": body.get("tickers", ["AAPL", "MSFT", "NVDA"]),
                    "created_at": "2026-08-29T12:00:00Z",
                    "updated_at": "2026-08-29T12:05:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        if url_path.startswith("/universes/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            uid = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "id": uid,
                    "status": "deleted",
                    "message": f"Universe '{uid}' was successfully deleted",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "93", "X-RateLimit-Reset": "54"},
            )

        # 4. GET /spillovers
        if url_path == "/spillovers" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "")
            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "spillovers": [
                        {
                            "related_ticker": "MSFT",
                            "lag_hours": 1,
                            "correlation": 0.745,
                            "relationship": f"{ticker} LEADS MSFT by 1h",
                            "updated_at": "2026-08-28T23:59:00Z",
                        }
                    ],
                    "count": 1,
                    "status": "ok",
                    "message": "Cross-asset spillover graph computed",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "97",
                    "X-RateLimit-Reset": "58",
                },
            )

        # 4b. GET /spillovers/matrix
        if url_path == "/spillovers/matrix" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            tickers_raw = request.url.params.get("tickers", "AAPL,MSFT,NVDA")
            tickers = [t.strip() for t in tickers_raw.split(",") if t.strip()]
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-03-31")
            min_corr = float(request.url.params.get("min_correlation", 0.0))
            max_lag = int(request.url.params.get("max_lag_hours", 24))

            matrix_items = [
                {
                    "ticker_a": "AAPL",
                    "ticker_b": "MSFT",
                    "correlation": 0.75,
                    "lag_hours": 1,
                    "direction": "AAPL_leads_MSFT",
                },
                {
                    "ticker_a": "AAPL",
                    "ticker_b": "NVDA",
                    "correlation": 0.68,
                    "lag_hours": -2,
                    "direction": "NVDA_leads_AAPL",
                },
                {
                    "ticker_a": "AAPL",
                    "ticker_b": "AAPL",
                    "correlation": 1.0,
                    "lag_hours": 0,
                    "direction": "self",
                },
            ]

            return httpx.Response(
                200,
                json={
                    "tickers": tickers,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_correlation": min_corr,
                    "max_lag_hours": max_lag,
                    "count": len(matrix_items),
                    "matrix": matrix_items,
                    "generated_at": "2026-08-28T20:15:00Z",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "96",
                    "X-RateLimit-Reset": "57",
                },
            )

        # 4c. GET /options/iv
        if url_path == "/options/iv" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            exp_date = request.url.params.get("expiration_date", "2025-12-19")
            strike_param = request.url.params.get("strike")
            opt_type = request.url.params.get("option_type", "all")

            if strike_param is not None:
                strike_val = float(strike_param)
                contracts = [
                    {
                        "ticker": f"O:{ticker}251219C00{int(strike_val*1000):06}",
                        "underlying_ticker": ticker,
                        "expiration_date": exp_date,
                        "strike": strike_val,
                        "option_type": opt_type.upper() if opt_type in ["call", "put"] else "CALL",
                        "bid": 8.45,
                        "ask": 8.65,
                        "last": 8.55,
                        "volume": 4500,
                        "open_interest": 22000,
                        "implied_volatility": 0.2850,
                        "delta": 0.4215,
                        "gamma": 0.0098,
                        "theta": -0.0452,
                        "vega": 0.2850,
                        "rho": 0.2450,
                    }
                ]
            else:
                contracts = [
                    {
                        "ticker": f"O:{ticker}251219C00220000",
                        "underlying_ticker": ticker,
                        "expiration_date": exp_date,
                        "strike": 220.0,
                        "option_type": "CALL",
                        "bid": 14.10,
                        "ask": 14.30,
                        "last": 14.20,
                        "volume": 6200,
                        "open_interest": 31000,
                        "implied_volatility": 0.2750,
                        "delta": 0.5820,
                        "gamma": 0.0105,
                        "theta": -0.0480,
                        "vega": 0.2910,
                        "rho": 0.2800,
                    },
                    {
                        "ticker": f"O:{ticker}251219P00220000",
                        "underlying_ticker": ticker,
                        "expiration_date": exp_date,
                        "strike": 220.0,
                        "option_type": "PUT",
                        "bid": 9.40,
                        "ask": 9.60,
                        "last": 9.50,
                        "volume": 5100,
                        "open_interest": 24000,
                        "implied_volatility": 0.2750,
                        "delta": -0.4180,
                        "gamma": 0.0105,
                        "theta": -0.0410,
                        "vega": 0.2910,
                        "rho": -0.2200,
                    },
                ]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "expiration_date": exp_date,
                    "underlying_price": 224.50,
                    "risk_free_rate": float(request.url.params.get("risk_free_rate", 0.05)),
                    "dividend_yield": float(request.url.params.get("dividend_yield", 0.0)),
                    "count": len(contracts),
                    "contracts": contracts,
                    "message": "Options IV and Greeks calculated successfully",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 4d. GET /options/unusual
        if url_path == "/options/unusual" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker_filter = request.url.params.get("ticker")
            min_ratio = float(request.url.params.get("min_volume_oi_ratio", 2.0))
            min_vol = int(request.url.params.get("min_volume", 100))
            days = int(request.url.params.get("days", 1))
            limit = int(request.url.params.get("limit", 20))

            all_items = [
                {
                    "ticker": "O:AAPL251219C00250000",
                    "underlying_ticker": "AAPL",
                    "expiration_date": "2025-12-19",
                    "strike": 250.0,
                    "option_type": "CALL",
                    "volume": 48500,
                    "open_interest": 2200,
                    "avg_volume": 3200.0,
                    "volume_oi_ratio": 22.0455,
                    "volume_zscore": 25.1667,
                    "score": 554.8119,
                    "timestamp": "2025-08-29T12:00:00Z",
                },
                {
                    "ticker": "O:NVDA251219C00140000",
                    "underlying_ticker": "NVDA",
                    "expiration_date": "2025-12-19",
                    "strike": 140.0,
                    "option_type": "CALL",
                    "volume": 95000,
                    "open_interest": 8000,
                    "avg_volume": 12000.0,
                    "volume_oi_ratio": 11.8750,
                    "volume_zscore": 15.0909,
                    "score": 179.2045,
                    "timestamp": "2025-08-29T12:00:00Z",
                },
            ]

            filtered = [
                item for item in all_items
                if (ticker_filter is None or item["underlying_ticker"] == ticker_filter.upper())
                and item["volume"] >= min_vol
                and item["volume_oi_ratio"] >= min_ratio
            ][:limit]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker_filter.upper() if ticker_filter else "ALL",
                    "min_volume_oi_ratio": min_ratio,
                    "min_volume": min_vol,
                    "days": days,
                    "count": len(filtered),
                    "items": filtered,
                    "message": f"Unusual options activity scan completed: {len(filtered)} contract(s) flagged",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 4e. GET /usage/stats
        if url_path == "/usage/stats" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            start_date = request.url.params.get("start_date", "2026-07-30")
            end_date = request.url.params.get("end_date", "2026-08-29")
            group_by = request.url.params.get("group_by", "day")
            limit = int(request.url.params.get("limit", 100))

            if group_by == "endpoint":
                breakdown = [
                    {"key": "/sentiment", "count": 560, "successful_requests": 540, "failed_requests": 20, "rate_limited_requests": 5, "average_latency_ms": 3.2, "p95_latency_ms": 8.5, "max_latency_ms": 24.0},
                    {"key": "/options/iv", "count": 250, "successful_requests": 245, "failed_requests": 5, "rate_limited_requests": 2, "average_latency_ms": 5.8, "p95_latency_ms": 16.0, "max_latency_ms": 42.0},
                ][:limit]
            elif group_by == "status_code":
                breakdown = [
                    {"key": "200", "count": 1205, "successful_requests": 1205, "failed_requests": 0, "rate_limited_requests": 0, "average_latency_ms": 4.2, "p95_latency_ms": 12.0, "max_latency_ms": 38.0},
                    {"key": "400", "count": 25, "successful_requests": 0, "failed_requests": 25, "rate_limited_requests": 0, "average_latency_ms": 1.2, "p95_latency_ms": 2.5, "max_latency_ms": 8.0},
                    {"key": "429", "count": 12, "successful_requests": 0, "failed_requests": 12, "rate_limited_requests": 12, "average_latency_ms": 0.8, "p95_latency_ms": 1.5, "max_latency_ms": 4.0},
                ][:limit]
            elif group_by == "method":
                breakdown = [
                    {"key": "GET", "count": 1180, "successful_requests": 1140, "failed_requests": 40, "rate_limited_requests": 10, "average_latency_ms": 4.2, "p95_latency_ms": 12.5, "max_latency_ms": 45.6},
                    {"key": "POST", "count": 70, "successful_requests": 65, "failed_requests": 5, "rate_limited_requests": 2, "average_latency_ms": 15.0, "p95_latency_ms": 38.0, "max_latency_ms": 120.0},
                ][:limit]
            else:  # day
                breakdown = [
                    {"key": "2026-08-28", "count": 42, "successful_requests": 40, "failed_requests": 2, "rate_limited_requests": 0, "average_latency_ms": 4.5, "p95_latency_ms": 13.5, "max_latency_ms": 38.0},
                    {"key": "2026-08-29", "count": 42, "successful_requests": 41, "failed_requests": 1, "rate_limited_requests": 0, "average_latency_ms": 4.6, "p95_latency_ms": 13.8, "max_latency_ms": 39.0},
                ][:limit]

            return httpx.Response(
                200,
                json={
                    "user_id": "test_quant_fund",
                    "start_date": start_date,
                    "end_date": end_date,
                    "group_by": group_by,
                    "summary": {
                        "total_requests": 1250,
                        "successful_requests": 1205,
                        "failed_requests": 45,
                        "rate_limited_requests": 12,
                        "average_latency_ms": 4.85,
                        "p95_latency_ms": 14.20,
                        "max_latency_ms": 45.60,
                    },
                    "breakdown": breakdown,
                    "message": "Usage statistics aggregated successfully",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 4f. GET /events/study
        if url_path == "/events/study" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            event_date = request.url.params.get("event_date", "2025-06-15")
            event_window = int(request.url.params.get("event_window", 5))
            estimation_window = int(request.url.params.get("estimation_window", 60))
            benchmark_ticker = request.url.params.get("benchmark_ticker", "SPY")

            abnormal_returns = []
            cum_ar = 0.0
            for offset in range(-event_window, event_window + 1):
                ar = 0.005 if offset == 0 else 0.001 * offset
                cum_ar += ar
                abnormal_returns.append({
                    "date": f"2025-06-{15 + offset:02d}",
                    "day_offset": offset,
                    "actual_return": 0.01 + ar,
                    "benchmark_return": 0.005,
                    "expected_return": 0.005 * 1.2,
                    "abnormal_return": round(ar, 6),
                    "cumulative_abnormal_return": round(cum_ar, 6),
                })

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper(),
                    "event_date": event_date,
                    "event_window": event_window,
                    "estimation_window": estimation_window,
                    "benchmark_ticker": benchmark_ticker.upper(),
                    "alpha": 0.0005,
                    "beta": 1.20,
                    "r_squared": 0.65,
                    "car_full_window": round(cum_ar, 6),
                    "car_pre_event": -0.015,
                    "car_post_event": 0.015,
                    "count": len(abnormal_returns),
                    "abnormal_returns": abnormal_returns,
                    "message": "Market model OLS regression over estimation window",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 4g. GET /events/8k
        if url_path == "/events/8k" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker_param = request.url.params.get("ticker")
            event_type_param = request.url.params.get("event_type")
            days_param = int(request.url.params.get("days", 7))
            limit_param = int(request.url.params.get("limit", 50))

            filings = [
                {
                    "ticker": "AAPL",
                    "filing_date": "2026-08-25",
                    "form_type": "8-K",
                    "event_type": "M&A",
                    "description": "Apple announces acquisition of AI startup for $1.2B.",
                    "items": ["1.01", "2.01"],
                    "accession_number": "0000320193-26-000085",
                    "url": "https://www.sec.gov/Archives/edgar/data/320193/000032019326000085/aapl-20260825.htm",
                },
                {
                    "ticker": "NVDA",
                    "filing_date": "2026-08-27",
                    "form_type": "8-K",
                    "event_type": "CEO Change",
                    "description": "NVIDIA announces appointment of new Chief Operating Officer.",
                    "items": ["5.02"],
                    "accession_number": "0001045810-26-000042",
                    "url": "https://www.sec.gov/Archives/edgar/data/1045810/000104581026000042/nvda-20260827.htm",
                },
                {
                    "ticker": "AAPL",
                    "filing_date": "2026-08-28",
                    "form_type": "8-K",
                    "event_type": "Earnings Warning",
                    "description": "Apple provides updated revenue guidance for upcoming quarter.",
                    "items": ["2.02", "7.01"],
                    "accession_number": "0000320193-26-000088",
                    "url": "https://www.sec.gov/Archives/edgar/data/320193/000032019326000088/aapl-20260828.htm",
                },
            ]

            filtered = [
                f for f in filings
                if (ticker_param is None or f["ticker"] == ticker_param.upper())
                and (event_type_param is None or f["event_type"].lower() == event_type_param.lower())
            ][:limit_param]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker_param.upper() if ticker_param else "ALL",
                    "event_type": event_type_param if event_type_param else "ALL",
                    "days": days_param,
                    "count": len(filtered),
                    "filings": filtered,
                    "message": "SEC Form 8-K filings retrieved and classified successfully",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "98",
                    "X-RateLimit-Reset": "59",
                },
            )

        # 5. POST /backtest
        if url_path == "/backtest" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content)
            tickers = body.get("tickers") or ([body["ticker"]] if "ticker" in body and body["ticker"] else ["AAPL"])
            weights = body.get("weights") or [1.0 / len(tickers)] * len(tickers)
            ticker_str = ", ".join(tickers) if len(tickers) > 1 else tickers[0]
            cost_bps = body.get("transaction_cost_bps", 5.0)

            return httpx.Response(
                200,
                json={
                    "ticker": ticker_str,
                    "tickers": tickers,
                    "weights": weights,
                    "benchmark_ticker": body.get("benchmark_ticker", "SPY"),
                    "start_date": body["start_date"],
                    "end_date": body["end_date"],
                    "total_return": 0.185,
                    "annualized_return": 0.62,
                    "sharpe_ratio": 2.45,
                    "sortino_ratio": 3.82,
                    "max_drawdown": -0.038,
                    "num_trades": 15,
                    "win_rate": 73.3,
                    "profit_factor": 2.14,
                    "transaction_cost_bps": cost_bps,
                    "equity_curve": [1000000.0, 1050000.0, 1185000.0],
                    "equity_points": [
                        {"date": "2025-01-01", "portfolio_value": 1000000.0, "daily_return": 0.0},
                        {"date": "2025-01-02", "portfolio_value": 1050000.0, "daily_return": 0.05},
                        {"date": "2025-01-03", "portfolio_value": 1185000.0, "daily_return": 0.1286},
                    ],
                    "benchmark_curve": [1000000.0, 1010000.0, 1025000.0],
                    "benchmark_total_return": 0.025,
                    "alpha": 0.160,
                    "message": "Backtest simulated successfully",
                },
                headers={
                    "X-RateLimit-Limit": "100",
                    "X-RateLimit-Remaining": "96",
                    "X-RateLimit-Reset": "57",
                },
            )

        # Billing endpoints
        if url_path == "/billing/checkout" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content)
            plan_id = body.get("plan_id")
            if plan_id not in ["pro_monthly", "enterprise_monthly"]:
                return httpx.Response(400, json={"error": "Bad Request", "message": f"Unknown plan '{plan_id}'"})

            return httpx.Response(
                200,
                json={
                    "checkout_url": f"https://checkout.stripe.com/mock/pay?plan={plan_id}",
                    "session_id": "cs_mock_1234567890",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/billing/portal" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "portal_url": "https://billing.stripe.com/mock/portal",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/billing/subscription" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "plan_id": "free",
                    "plan_name": "Free Tier",
                    "status": "active",
                    "monthly_request_limit": 1000,
                    "current_usage": 42,
                    "current_period_start": None,
                    "current_period_end": None,
                    "stripe_customer_id": None,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Organizations & Teams routes ───────────────────────────────────
        if url_path == "/orgs" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content)
            return httpx.Response(
                201,
                json={
                    "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "name": body.get("name", "Test Org"),
                    "role": "admin",
                    "created_at": "2026-08-30T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/orgs" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "organizations": [
                        {
                            "id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                            "name": "Acme Trading",
                            "created_by": "trader_007",
                            "created_at": "2026-08-30T12:00:00Z",
                            "member_count": 3,
                            "my_role": "admin",
                        }
                    ],
                    "count": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        import re as _re

        org_detail_match = _re.match(r"^/orgs/([^/]+)$", url_path)
        if org_detail_match and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = org_detail_match.group(1)
            return httpx.Response(
                200,
                json={
                    "id": oid,
                    "name": "Acme Trading",
                    "created_by": "trader_007",
                    "created_at": "2026-08-30T12:00:00Z",
                    "members": [
                        {"user_id": "trader_007", "role": "admin", "joined_at": "2026-08-30T12:00:00Z"},
                        {"user_id": "analyst_42", "role": "member", "joined_at": "2026-08-30T12:05:00Z"},
                    ],
                    "count": 2,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        invite_match = _re.match(r"^/orgs/([^/]+)/invites$", url_path)
        if invite_match and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = invite_match.group(1)
            body = json.loads(request.content)
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "message": f"User {body['user_id']} added to organization",
                    "org_id": oid,
                    "user_id": body["user_id"],
                    "role": body.get("role", "member"),
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        update_role_match = _re.match(r"^/orgs/([^/]+)/members/([^/]+)$", url_path)
        if update_role_match and method == "PATCH":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = update_role_match.group(1)
            uid = update_role_match.group(2)
            body = json.loads(request.content)
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "message": f"Role updated to {body['role']}",
                    "org_id": oid,
                    "user_id": uid,
                    "role": body["role"],
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        remove_match = _re.match(r"^/orgs/([^/]+)/members/([^/]+)$", url_path)
        if remove_match and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = remove_match.group(1)
            uid = remove_match.group(2)
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "message": f"User {uid} removed from organization",
                    "org_id": oid,
                    "user_id": uid,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        leave_match = _re.match(r"^/orgs/([^/]+)/leave$", url_path)
        if leave_match and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = leave_match.group(1)
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "message": "Left organization successfully",
                    "org_id": oid,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        select_match = _re.match(r"^/orgs/([^/]+)/select$", url_path)
        if select_match and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            oid = select_match.group(1)
            return httpx.Response(
                200,
                json={
                    "status": "ok",
                    "message": "Organization context activated",
                    "token": f"jwt_org_scoped_{oid}",
                    "org_id": oid,
                    "role": "admin",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Security & IP Whitelist routes ─────────────────────────────────
        if url_path == "/security/ip-whitelist" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "entries": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440000",
                            "user_id": "test_user",
                            "ip_or_cidr": "203.0.113.0/24",
                            "description": "Primary Office VPN",
                            "created_at": "2026-08-30T12:00:00Z",
                        }
                    ],
                    "count": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/security/ip-whitelist" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content)
            cidr = body.get("ip_or_cidr", "")
            if "invalid" in cidr:
                return httpx.Response(400, json={"error": "Bad Request", "message": f"Invalid IP address or CIDR format: '{cidr}'"})
            return httpx.Response(
                201,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "user_id": "test_user",
                    "ip_or_cidr": cidr if "/" in cidr else f"{cidr}/32",
                    "description": body.get("description"),
                    "created_at": "2026-08-30T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        del_ip_match = _re.match(r"^/security/ip-whitelist/([^/]+)$", url_path)
        if del_ip_match and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            entry_id = del_ip_match.group(1)
            if entry_id == "non_existent_id":
                return httpx.Response(404, json={"error": "Not Found", "message": f"IP whitelist entry '{entry_id}' not found"})
            return httpx.Response(
                200,
                json={
                    "status": "success",
                    "message": "IP whitelist entry deleted successfully",
                    "id": entry_id,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── News Articles routes ───────────────────────────────────────────
        if url_path == "/news/articles" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            ticker = request.url.params.get("ticker", "AAPL")
            return httpx.Response(
                200,
                json={
                    "articles": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440000",
                            "ticker": ticker,
                            "title": f"{ticker} Reports Robust Enterprise Expansion",
                            "source": "Institutional Wire",
                            "published_utc": "2026-08-30T10:00:00Z",
                            "sentiment_score": 0.82,
                            "sentiment_label": "positive",
                            "confidence": 0.94,
                            "data_quality_score": 0.98,
                            "snippet": f"{ticker} announced accelerated revenue momentum across institutional subscriptions and enterprise partnerships...",
                        }
                    ],
                    "total": 1,
                    "limit": 20,
                    "offset": 0,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        article_match = _re.match(r"^/news/articles/([^/]+)$", url_path)
        if article_match and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            art_id = article_match.group(1)
            if art_id == "non_existent_id":
                return httpx.Response(404, json={"error": "Not Found", "message": f"News article '{art_id}' not found"})
            return httpx.Response(
                200,
                json={
                    "id": art_id,
                    "ticker": "AAPL",
                    "title": "Apple Unveils Next-Gen Neural Engine & Services Growth Outlook",
                    "source": "Institutional Wire",
                    "published_utc": "2026-08-30T10:00:00Z",
                    "full_text": "Apple Inc. announced significant expansion of its enterprise cloud compute partnerships and next-generation neural processing capabilities during its institutional investor briefing in Cupertino. Senior leadership emphasized double-digit growth in high-margin services revenue and accelerated enterprise hardware refresh cycles.",
                    "sentiment_score": 0.84,
                    "sentiment_label": "positive",
                    "confidence": 0.95,
                    "data_quality_score": 0.98,
                    "created_at": "2026-08-30T10:00:05Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Entity Sentiment Breakdown route ────────────────────────────────
        if url_path == "/sentiment/entities" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            e_type = request.url.params.get("entity_type", "all")
            min_m = int(request.url.params.get("min_mentions", 5))
            if min_m > 50:
                return httpx.Response(200, json={
                    "start_date": "2026-08-23",
                    "end_date": "2026-08-30",
                    "entity_type": e_type,
                    "min_mentions": min_m,
                    "count": 0,
                    "entities": [],
                    "generated_at": "2026-08-30T12:00:00Z"
                }, headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"})
            return httpx.Response(
                200,
                json={
                    "start_date": "2026-08-23",
                    "end_date": "2026-08-30",
                    "entity_type": e_type,
                    "min_mentions": min_m,
                    "count": 2,
                    "entities": [
                        {
                            "entity_text": "Nvidia" if e_type in ["all", "company"] else "Jensen Huang",
                            "entity_type": "company" if e_type in ["all", "company"] else "person",
                            "avg_sentiment": 0.78,
                            "positive_ratio": 0.90,
                            "negative_ratio": 0.05,
                            "mention_count": 14,
                            "latest_mention_date": "2026-08-30",
                        },
                        {
                            "entity_text": "Apple" if e_type in ["all", "company"] else "Tim Cook",
                            "entity_type": "company" if e_type in ["all", "company"] else "person",
                            "avg_sentiment": 0.45,
                            "positive_ratio": 0.75,
                            "negative_ratio": 0.10,
                            "mention_count": 10,
                            "latest_mention_date": "2026-08-29",
                        },
                    ],
                    "generated_at": "2026-08-30T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Compliance Audit Log routes ─────────────────────────────────────
        if url_path == "/audit/logs" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            action_filter = request.url.params.get("action")
            user_filter = request.url.params.get("user_id")
            org_filter = request.url.params.get("org_id")
            limit = int(request.url.params.get("limit", 100))
            offset = int(request.url.params.get("offset", 0))

            all_logs = [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440001",
                    "org_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "user_id": "trader_007",
                    "action": "apikey.create",
                    "entity_type": "api_key",
                    "entity_id": "key_uuid_123",
                    "details": {"name": "Prod Key", "prefix": "ft_live_"},
                    "ip_address": "192.168.1.10",
                    "created_at": "2026-08-30T12:00:00Z",
                },
                {
                    "id": "550e8400-e29b-41d4-a716-446655440002",
                    "org_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "user_id": "trader_007",
                    "action": "org.created",
                    "entity_type": "organization",
                    "entity_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                    "details": {"name": "Acme Trading"},
                    "ip_address": "192.168.1.10",
                    "created_at": "2026-08-30T11:55:00Z",
                },
            ]

            filtered = [
                l for l in all_logs
                if (action_filter is None or l["action"] == action_filter)
                and (user_filter is None or l["user_id"] == user_filter)
                and (org_filter is None or l["org_id"] == org_filter)
            ]

            paged = filtered[offset : offset + limit]
            return httpx.Response(
                200,
                json={
                    "logs": paged,
                    "total": len(filtered),
                    "limit": limit,
                    "offset": offset,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/audit/export" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            fmt = request.url.params.get("format", "json").lower()
            if fmt == "csv":
                csv_data = "id,org_id,user_id,action,entity_type,entity_id,details,ip_address,created_at\n550e8400-e29b-41d4-a716-446655440001,a1b2c3d4-e5f6-7890-abcd-ef1234567890,trader_007,apikey.create,api_key,key_uuid_123,\"{\"\"name\"\":\"\"Prod Key\"\"}\",192.168.1.10,2026-08-30T12:00:00Z\n"
                return httpx.Response(
                    200,
                    text=csv_data,
                    headers={
                        "Content-Type": "text/csv; charset=utf-8",
                        "Content-Disposition": 'attachment; filename="fintext_audit_logs.csv"',
                        "X-RateLimit-Limit": "100",
                        "X-RateLimit-Remaining": "99",
                        "X-RateLimit-Reset": "60",
                    },
                )
            else:
                return httpx.Response(
                    200,
                    json={
                        "exported_at": "2026-08-30T12:05:00Z",
                        "total": 1,
                        "logs": [
                            {
                                "id": "550e8400-e29b-41d4-a716-446655440001",
                                "org_id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
                                "user_id": "trader_007",
                                "action": "apikey.create",
                                "entity_type": "api_key",
                                "entity_id": "key_uuid_123",
                                "details": {"name": "Prod Key", "prefix": "ft_live_"},
                                "ip_address": "192.168.1.10",
                                "created_at": "2026-08-30T12:00:00Z",
                            }
                        ],
                    },
                    headers={
                        "Content-Type": "application/json; charset=utf-8",
                        "X-RateLimit-Limit": "100",
                        "X-RateLimit-Remaining": "99",
                        "X-RateLimit-Reset": "60",
                    },
                )

        # 26. API Key endpoints
        if url_path == "/auth/api-keys" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            name = body.get("name", "Default Key")
            return httpx.Response(
                201,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440001",
                    "name": name,
                    "api_key": "ft_1234567890abcdef1234567890abcdef",
                    "prefix": "ft_12345",
                    "created_at": "2026-08-30T12:00:00Z",
                },
            )

        if url_path == "/auth/api-keys" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "api_keys": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440001",
                            "name": "Prod Bot",
                            "prefix": "ft_12345",
                            "created_at": "2026-08-30T12:00:00Z",
                            "revoked_at": None,
                            "expires_at": "2026-08-31T12:00:00Z",
                            "rotated_from": None,
                            "rotation_status": "rotating",
                        }
                    ],
                    "count": 1,
                },
            )

        if url_path == "/auth/api-keys/550e8400-e29b-41d4-a716-446655440001" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440001",
                    "name": "Prod Bot",
                    "prefix": "ft_12345",
                    "created_at": "2026-08-30T12:00:00Z",
                    "revoked_at": None,
                    "expires_at": "2026-08-31T12:00:00Z",
                    "rotated_from": None,
                    "rotation_status": "rotating",
                },
            )

        if url_path == "/auth/api-keys/550e8400-e29b-41d4-a716-446655440001/rotate" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            overlap = body.get("overlap_hours", 24)
            return httpx.Response(
                200,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440002",
                    "name": "Prod Bot (Rotated)",
                    "api_key": "ft_9876543210fedcba9876543210fedcba",
                    "prefix": "ft_98765",
                    "created_at": "2026-08-30T12:00:00Z",
                    "rotated_from": "550e8400-e29b-41d4-a716-446655440001",
                    "old_key_id": "550e8400-e29b-41d4-a716-446655440001",
                    "old_key_expires_at": "2026-08-31T12:00:00Z",
                    "overlap_hours": overlap,
                },
            )

        if url_path == "/auth/api-keys/550e8400-e29b-41d4-a716-446655440001" and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440001",
                    "status": "revoked",
                    "message": "API key revoked successfully",
                },
            )

        # 27. GET /search
        if url_path == "/search" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            q = request.url.params.get("q", "")
            if not q or not q.strip():
                return httpx.Response(400, json={"error": "Bad Request", "message": "Search query 'q' cannot be empty."})

            limit = int(request.url.params.get("limit", 20))
            if limit < 1 or limit > 100:
                return httpx.Response(400, json={"error": "Bad Request", "message": "limit must be between 1 and 100."})

            offset = int(request.url.params.get("offset", 0))
            types_param = request.url.params.get("types", "all")
            selected_types = [t.strip() for t in types_param.split(",")] if types_param != "all" else ["news", "sentiment", "transcripts", "filings", "events", "insider", "supply_chain", "options"]

            for t in selected_types:
                if t not in ["news", "sentiment", "transcripts", "filings", "events", "insider", "supply_chain", "options"]:
                    return httpx.Response(400, json={"error": "Bad Request", "message": f"Invalid data type '{t}'."})

            results = [
                {
                    "id": "mock_search_1",
                    "type": "news",
                    "ticker": "AAPL",
                    "title": "Apple reports record quarterly revenue",
                    "snippet": "Apple Inc. announced financial results for Q4...",
                    "date": "2025-08-15T10:00:00Z",
                    "score": 1.0 if q.upper() == "AAPL" else 0.8,
                },
                {
                    "id": "mock_search_2",
                    "type": "transcript",
                    "ticker": "AAPL",
                    "title": "Apple Inc. Q3 2025 Earnings Call Transcript",
                    "snippet": "Tim Cook: Good afternoon everyone...",
                    "date": "2025-07-31",
                    "score": 1.0 if q.upper() == "AAPL" else 0.8,
                },
            ]

            paged = results[offset : offset + limit]
            return httpx.Response(
                200,
                json={
                    "query": q,
                    "types": selected_types,
                    "count": len(paged),
                    "results": paged,
                    "generated_at": "2026-08-30T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /stream/kafka/topics
        if url_path == "/stream/kafka/topics" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "topics": [
                        {
                            "topic": "sentiment-events",
                            "description": "Real-time institutional sentiment scores",
                            "schema_description": "JSON events with ticker, sentiment_score, sentiment_label",
                            "example_payload": {"ticker": "AAPL", "sentiment_score": 0.85},
                            "partitions": 12,
                            "retention_hours": 168,
                        },
                        {
                            "topic": "news-events",
                            "description": "Curated news headlines",
                            "schema_description": "JSON events with ticker, title, source",
                            "example_payload": {"ticker": "NVDA", "title": "Headline"},
                            "partitions": 8,
                            "retention_hours": 168,
                        },
                        {
                            "topic": "options-events",
                            "description": "Real-time options IV and Greeks",
                            "schema_description": "JSON events with ticker, strike, iv",
                            "example_payload": {"ticker": "MSFT", "strike": 450.0},
                            "partitions": 16,
                            "retention_hours": 72,
                        },
                    ],
                    "total_topics": 3,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /stream/kafka/credentials
        if url_path == "/stream/kafka/credentials" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            topic = request.url.params.get("topic", "sentiment-events")
            ttl = int(request.url.params.get("ttl_minutes", "60"))
            cg = request.url.params.get("consumer_group", "cg-user-sentiment")
            return httpx.Response(
                200,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "user_id": "test_quant_fund",
                    "username": "user_550e8400e29b41d4a716446655440000",
                    "password": "sec_k9x2m4p8q1w7r3t5y8u2i4o6p9a1s3d5",
                    "broker_address": "127.0.0.1:9092",
                    "topic": topic,
                    "consumer_group": cg,
                    "issued_at": "2026-08-31T10:00:00Z",
                    "expires_at": "2026-08-31T11:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # DELETE /stream/kafka/credentials/{id}
        if url_path.startswith("/stream/kafka/credentials/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            cid = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "status": "revoked",
                    "message": "Kafka credentials revoked successfully",
                    "id": cid,
                    "revoked_at": "2026-08-31T10:30:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /retention/policies
        if url_path == "/retention/policies" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "policies": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440001",
                            "org_id": None,
                            "user_id": "test_quant_fund",
                            "data_category": "usage_events",
                            "retention_days": 90,
                            "is_active": True,
                            "created_at": "2026-08-31T10:00:00Z",
                            "updated_at": "2026-08-31T10:00:00Z",
                        }
                    ],
                    "total_policies": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /retention/policies
        if url_path == "/retention/policies" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content)
            cat = body.get("data_category", "usage_events")
            days = int(body.get("retention_days", 90))
            is_active = bool(body.get("is_active", True))
            return httpx.Response(
                200,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440002",
                    "org_id": None,
                    "user_id": "test_quant_fund",
                    "data_category": cat,
                    "retention_days": days,
                    "is_active": is_active,
                    "created_at": "2026-08-31T10:00:00Z",
                    "updated_at": "2026-08-31T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # DELETE /retention/policies/{id}
        if url_path.startswith("/retention/policies/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            pid = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "status": "deleted",
                    "message": "Retention policy deleted successfully",
                    "id": pid,
                    "deleted_at": "2026-08-31T10:30:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /risk/factor-exposure
        if url_path == "/risk/factor-exposure" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            ticker = request.url.params.get("ticker", "AAPL")
            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-06-30")
            factors_param = request.url.params.get("factors", "market,momentum,sentiment,volatility")
            factors_list = [f.strip() for f in factors_param.split(",") if f.strip()]
            benchmark_ticker = request.url.params.get("benchmark_ticker", "SPY")

            exposures = []
            for f in factors_list:
                exposures.append({
                    "factor": f,
                    "beta": 1.15 if f == "market" else 0.20,
                    "t_stat": 8.5 if f == "market" else 2.3,
                    "p_value": 0.001 if f == "market" else 0.02,
                })

            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "start_date": start_date,
                    "end_date": end_date,
                    "benchmark_ticker": benchmark_ticker,
                    "factors_included": factors_list,
                    "ols_summary": {
                        "r_squared": 0.72,
                        "adjusted_r_squared": 0.70,
                        "num_observations": 120,
                        "f_statistic": 45.3,
                        "p_value": 0.0001,
                    },
                    "exposures": exposures,
                    "generated_at": "2026-08-31T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /esg/scores
        if url_path == "/esg/scores" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            ticker = request.url.params.get("ticker")
            sector = request.url.params.get("sector")
            start_date = request.url.params.get("start_date", "2025-06-01")
            end_date = request.url.params.get("end_date", "2025-08-30")
            min_confidence = float(request.url.params.get("min_confidence", "0.0"))

            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "sector": sector,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_confidence": min_confidence,
                    "overall_esg_score": 68.5,
                    "dimensions": {
                        "environmental": {
                            "score": 0.42,
                            "mention_count": 12,
                            "positive_ratio": 0.75,
                            "negative_ratio": 0.08,
                        },
                        "social": {
                            "score": 0.35,
                            "mention_count": 15,
                            "positive_ratio": 0.67,
                            "negative_ratio": 0.13,
                        },
                        "governance": {
                            "score": 0.38,
                            "mention_count": 10,
                            "positive_ratio": 0.70,
                            "negative_ratio": 0.10,
                        },
                    },
                    "generated_at": "2026-08-31T11:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /risk/bankruptcy
        if url_path == "/risk/bankruptcy" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            ticker = request.url.params.get("ticker", "AAPL")
            lookback_days = int(request.url.params.get("lookback_days", "30"))
            include_components = request.url.params.get("include_components", "true").lower() == "true"

            components = None
            if include_components:
                components = {
                    "eight_k_distress_score": 10.0,
                    "sentiment_deterioration_score": 5.0,
                    "put_call_ratio_score": 4.5,
                    "implied_volatility_score": 6.0,
                    "supply_chain_risk_score": 3.0,
                    "insider_selling_score": 4.0,
                }

            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "lookback_days": lookback_days,
                    "bankruptcy_risk_score": 32.5,
                    "risk_category": "MODERATE",
                    "components": components,
                    "generated_at": "2026-08-31T11:15:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /fx/sentiment
        if url_path == "/fx/sentiment" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            pair = request.url.params.get("currency_pair", "EUR/USD")
            start_date = request.url.params.get("start_date", "2025-08-01")
            end_date = request.url.params.get("end_date", "2025-08-31")
            min_confidence = float(request.url.params.get("min_confidence", "0.0"))

            return httpx.Response(
                200,
                json={
                    "currency_pair": pair,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_confidence": min_confidence,
                    "summary": {
                        "avg_sentiment": 0.28,
                        "mention_count": 18,
                        "positive_ratio": 0.65,
                        "negative_ratio": 0.15,
                        "latest_article_date": "2025-08-30T14:30:00Z",
                    },
                    "top_articles": [
                        {
                            "id": "11111111-2222-3333-4444-555555555555",
                            "title": "ECB signals potential rate cut as Eurozone inflation cools",
                            "source": "Reuters FX",
                            "published_utc": "2025-08-30T14:30:00Z",
                            "sentiment_score": 0.42,
                            "confidence": 0.91,
                            "url": "https://news.fintext.io/fx/eur-usd/ecb-signals-rate-cut",
                        }
                    ],
                    "generated_at": "2026-08-31T11:30:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /commodities/sentiment
        if url_path == "/commodities/sentiment" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            comm = request.url.params.get("commodity", "crude_oil")
            start_date = request.url.params.get("start_date", "2025-08-01")
            end_date = request.url.params.get("end_date", "2025-08-31")
            min_confidence = float(request.url.params.get("min_confidence", "0.0"))

            return httpx.Response(
                200,
                json={
                    "commodity": comm,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_confidence": min_confidence,
                    "summary": {
                        "avg_sentiment": 0.35,
                        "mention_count": 25,
                        "positive_ratio": 0.60,
                        "negative_ratio": 0.20,
                        "latest_article_date": "2025-08-30T10:00:00Z",
                    },
                    "top_articles": [
                        {
                            "id": "33333333-4444-5555-6666-777777777777",
                            "title": "OPEC+ confirms continuation of voluntary output cuts",
                            "source": "Reuters Commodities",
                            "published_utc": "2025-08-30T10:00:00Z",
                            "sentiment_score": 0.45,
                            "confidence": 0.92,
                            "url": "https://news.fintext.io/commodities/crude_oil/opec-cuts",
                        }
                    ],
                    "generated_at": "2026-08-31T11:45:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /crypto/sentiment
        if url_path == "/crypto/sentiment" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            asset = request.url.params.get("asset", "BTC")
            start_date = request.url.params.get("start_date", "2025-08-01")
            end_date = request.url.params.get("end_date", "2025-08-31")
            min_confidence = float(request.url.params.get("min_confidence", 0.0))

            return httpx.Response(
                200,
                json={
                    "asset": asset,
                    "start_date": start_date,
                    "end_date": end_date,
                    "min_confidence": min_confidence,
                    "summary": {
                        "avg_sentiment": 0.58,
                        "mention_count": 32,
                        "positive_ratio": 0.72,
                        "negative_ratio": 0.12,
                        "latest_article_date": "2025-08-30T14:00:00Z",
                    },
                    "top_articles": [
                        {
                            "id": "55555555-6666-7777-8888-999999999999",
                            "title": "Bitcoin spot ETFs record $450M in daily net institutional inflows",
                            "source": "CoinDesk",
                            "published_utc": "2025-08-30T14:00:00Z",
                            "sentiment_score": 0.65,
                            "confidence": 0.94,
                            "url": "https://news.fintext.io/crypto/btc/article-1",
                        }
                    ],
                    "generated_at": "2026-08-31T11:45:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /options/microstructure
        if url_path == "/options/microstructure" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            ticker = request.url.params.get("ticker", "AAPL")
            start_date = request.url.params.get("start_date", "2025-08-01")
            end_date = request.url.params.get("end_date", "2025-08-31")
            metric = request.url.params.get("metric", "both")
            interval = request.url.params.get("interval", "daily")
            limit = int(request.url.params.get("limit", 100))

            points = [
                {
                    "timestamp": "2025-08-01T20:00:00Z",
                    "vpin": 0.235,
                    "gex": 1250000.0,
                    "total_volume": 450000,
                    "trade_count": 8200,
                },
                {
                    "timestamp": "2025-08-02T20:00:00Z",
                    "vpin": 0.248,
                    "gex": 1310000.0,
                    "total_volume": 480000,
                    "trade_count": 8900,
                },
            ]

            return httpx.Response(
                200,
                json={
                    "ticker": ticker.upper(),
                    "start_date": start_date,
                    "end_date": end_date,
                    "metric": metric,
                    "interval": interval,
                    "count": len(points),
                    "points": points,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /market/breadth
        if url_path == "/market/breadth" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            start_date = request.url.params.get("start_date", "2025-01-01")
            end_date = request.url.params.get("end_date", "2025-01-15")
            universe = request.url.params.get("universe", "all")
            inc_52w = request.url.params.get("include_new_highs_lows", "true").lower() == "true"

            points = [
                {
                    "date": "2025-01-02",
                    "advancers": 300,
                    "decliners": 180,
                    "unchanged": 20,
                    "advance_decline_ratio": 0.625,
                    "breadth_index": 0.24,
                    "new_52w_highs": 25 if inc_52w else None,
                    "new_52w_lows": 10 if inc_52w else None,
                },
                {
                    "date": "2025-01-03",
                    "advancers": 250,
                    "decliners": 230,
                    "unchanged": 20,
                    "advance_decline_ratio": 0.521,
                    "breadth_index": 0.04,
                    "new_52w_highs": 15 if inc_52w else None,
                    "new_52w_lows": 20 if inc_52w else None,
                },
            ]

            return httpx.Response(
                200,
                json={
                    "start_date": start_date,
                    "end_date": end_date,
                    "universe": universe,
                    "total_tickers": 500,
                    "count": len(points),
                    "points": points,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /polling-webhooks
        if url_path == "/polling-webhooks" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            data = json.loads(request.content.decode("utf-8"))
            return httpx.Response(
                201,
                json={
                    "id": "77777777-8888-9999-aaaa-bbbbbbbbbbbb",
                    "user_id": "test_user",
                    "name": data.get("name", "Test Poll"),
                    "url": data.get("url", "https://quant.fund.com/poll"),
                    "interval_seconds": data.get("interval_seconds", 300),
                    "query_type": data.get("query_type", "sentiment"),
                    "query_params": data.get("query_params", {}),
                    "secret": "a" * 64,
                    "is_active": True,
                    "last_triggered_at": None,
                    "created_at": "2026-08-31T14:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /polling-webhooks
        if url_path == "/polling-webhooks" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "webhooks": [
                        {
                            "id": "77777777-8888-9999-aaaa-bbbbbbbbbbbb",
                            "user_id": "test_user",
                            "name": "Test Poll",
                            "url": "https://quant.fund.com/poll",
                            "interval_seconds": 300,
                            "query_type": "sentiment",
                            "query_params": {"tickers": ["AAPL", "MSFT"]},
                            "secret": "a" * 64,
                            "is_active": True,
                            "last_triggered_at": None,
                            "created_at": "2026-08-31T14:00:00Z",
                        }
                    ],
                    "total": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # DELETE /polling-webhooks/{id}
        if url_path.startswith("/polling-webhooks/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            wh_id = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "success": True,
                    "id": wh_id,
                    "message": "Polling webhook successfully deleted",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /chat-alerts
        if url_path == "/chat-alerts" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            data = json.loads(request.content.decode("utf-8"))
            return httpx.Response(
                201,
                json={
                    "id": "550e8400-e29b-41d4-a716-446655440000",
                    "user_id": "test_user",
                    "channel_type": data.get("channel_type", "telegram"),
                    "channel_target": data.get("channel_target", "123456789"),
                    "event_types": data.get("event_types", ["sentiment_anomaly"]),
                    "is_active": True,
                    "created_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /chat-alerts
        if url_path == "/chat-alerts" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            return httpx.Response(
                200,
                json={
                    "subscriptions": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440000",
                            "user_id": "test_user",
                            "channel_type": "telegram",
                            "channel_target": "123456789",
                            "event_types": ["sentiment_anomaly", "8k_filing"],
                            "is_active": True,
                            "created_at": "2026-08-31T15:00:00Z",
                        }
                    ],
                    "total": 1,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # DELETE /chat-alerts/{id}
        if url_path.startswith("/chat-alerts/") and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            sub_id = url_path.split("/")[-1]
            return httpx.Response(
                200,
                json={
                    "success": True,
                    "id": sub_id,
                    "message": "Chat alert subscription deleted successfully",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # GET /risk/credit-sentiment
        if url_path == "/risk/credit-sentiment" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            ticker = request.url.params.get("ticker", "AAPL")
            lookback = int(request.url.params.get("lookback_days", 30))
            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "lookback_days": lookback,
                    "credit_sentiment_score": 0.25,
                    "news_sentiment_avg": 0.35,
                    "8k_distress_count": 0,
                    "put_call_ratio": 0.75,
                    "implied_volatility": 0.28,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /sentiment/backfill
        if url_path == "/sentiment/backfill" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            ticker = body.get("ticker", "AAPL")
            start_date = body.get("start_date", "2025-01-01")
            end_date = body.get("end_date", "2025-03-31")
            overwrite = body.get("overwrite", False)
            return httpx.Response(
                200,
                json={
                    "ticker": ticker,
                    "start_date": start_date,
                    "end_date": end_date,
                    "overwrite": overwrite,
                    "total_articles_found": 150,
                    "processed_articles": 150,
                    "failed_articles": 0,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /portfolio/optimize
        if url_path == "/portfolio/optimize" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            tickers = body.get("tickers", ["AAPL", "MSFT", "NVDA"])
            start_date = body.get("start_date", "2025-01-01")
            end_date = body.get("end_date", "2025-06-30")
            opt_type = body.get("optimization_type", "max_sharpe")
            rf = body.get("risk_free_rate", 0.05)
            weights = [{"ticker": t, "weight": round(1.0 / len(tickers), 4)} for t in tickers]
            return httpx.Response(
                200,
                json={
                    "tickers": tickers,
                    "start_date": start_date,
                    "end_date": end_date,
                    "optimization_type": opt_type,
                    "risk_free_rate": rf,
                    "weights": weights,
                    "expected_annual_return": 0.15,
                    "expected_annual_volatility": 0.18,
                    "sharpe_ratio": 0.55,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # POST /risk/portfolio-factor-exposure
        if url_path == "/risk/portfolio-factor-exposure" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            tickers = body.get("tickers", ["AAPL", "MSFT", "NVDA"])
            weights = body.get("weights", [0.4, 0.3, 0.3])
            start_date = body.get("start_date", "2025-01-01")
            end_date = body.get("end_date", "2025-06-30")
            benchmark = body.get("benchmark_ticker", "SPY")
            factors_str = body.get("factors", "market,momentum,sentiment,volatility")
            factors_inc = [f.strip() for f in factors_str.split(",") if f.strip()]
            exposures = [
                {"factor": "market", "beta": 1.05, "t_stat": 15.2, "p_value": 0.001},
                {"factor": "momentum", "beta": 0.06, "t_stat": 2.8, "p_value": 0.01},
                {"factor": "sentiment", "beta": 0.18, "t_stat": 4.1, "p_value": 0.001},
                {"factor": "volatility", "beta": -0.28, "t_stat": -5.5, "p_value": 0.001},
            ]
            return httpx.Response(
                200,
                json={
                    "tickers": tickers,
                    "weights": weights,
                    "start_date": start_date,
                    "end_date": end_date,
                    "benchmark_ticker": benchmark,
                    "factors_included": factors_inc,
                    "ols_summary": {
                        "r_squared": 0.75,
                        "adjusted_r_squared": 0.74,
                        "num_observations": 120,
                        "f_statistic": 86.5,
                        "p_value": 0.0001,
                    },
                    "exposures": exposures,
                    "generated_at": "2026-08-31T15:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Retraining Jobs routes ──────────────────────────────────────────
        if url_path == "/retraining/jobs" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            model_type = body.get("model_type", "sentiment")
            trigger_type = body.get("trigger_type", "manual")
            config_payload = body.get("config", {})
            return httpx.Response(
                201,
                json={
                    "job": {
                        "id": "11111111-2222-3333-4444-555555555555",
                        "user_id": "test_user",
                        "model_type": model_type,
                        "status": "pending",
                        "trigger_type": trigger_type,
                        "created_at": "2026-08-31T12:00:00Z",
                        "config": config_payload,
                    },
                    "message": "Retraining job queued successfully",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/retraining/jobs" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            status_filter = request.url.params.get("status")
            return httpx.Response(
                200,
                json={
                    "jobs": [
                        {
                            "id": "11111111-2222-3333-4444-555555555555",
                            "user_id": "test_user",
                            "model_type": "sentiment",
                            "status": status_filter if status_filter else "completed",
                            "trigger_type": "manual",
                            "created_at": "2026-08-31T12:00:00Z",
                            "completed_at": "2026-08-31T12:00:05Z",
                            "config": {"epochs": 3, "batch_size": 32, "metrics": {"loss": 0.042, "f1_score": 0.945, "accuracy": 0.952}},
                        }
                    ],
                    "total": 1,
                    "limit": 50,
                    "offset": 0,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        retraining_cancel_match = _re.match(r"^/retraining/jobs/([^/]+)/cancel$", url_path)
        if retraining_cancel_match and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            job_id = retraining_cancel_match.group(1)
            if job_id == "non_existent_id":
                return httpx.Response(404, json={"error": "Not Found", "message": "Job not found"})
            return httpx.Response(
                200,
                json={
                    "job": {
                        "id": job_id,
                        "user_id": "test_user",
                        "model_type": "sentiment",
                        "status": "cancelled",
                        "trigger_type": "manual",
                        "created_at": "2026-08-31T12:00:00Z",
                        "completed_at": "2026-08-31T12:00:02Z",
                        "config": {},
                    },
                    "message": "Retraining job cancelled successfully",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        retraining_get_match = _re.match(r"^/retraining/jobs/([^/]+)$", url_path)
        if retraining_get_match and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})
            job_id = retraining_get_match.group(1)
            if job_id == "non_existent_id":
                return httpx.Response(404, json={"error": "Not Found", "message": "Job not found"})
            return httpx.Response(
                200,
                json={
                    "job": {
                        "id": job_id,
                        "user_id": "test_user",
                        "model_type": "sentiment",
                        "status": "completed",
                        "trigger_type": "manual",
                        "created_at": "2026-08-31T12:00:00Z",
                        "completed_at": "2026-08-31T12:00:05Z",
                        "config": {"epochs": 3, "batch_size": 32, "metrics": {"loss": 0.042, "f1_score": 0.945, "accuracy": 0.952}},
                    },
                    "message": "Retraining job retrieved successfully",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── FIX Protocol Bridge routes ──────────────────────────────────────
        if url_path == "/fix/order" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            msg = body.get("fix_message", "")
            if not msg or "35=D" not in msg:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid FIX NewOrderSingle message"})

            return httpx.Response(
                200,
                json={
                    "order_id": "f1000000-0000-0000-0000-000000000001",
                    "exec_id": "e1000000-0000-0000-0000-000000000001",
                    "cl_ord_id": "ORD-12345",
                    "symbol": "AAPL",
                    "side": "1",
                    "order_type": "1",
                    "qty": 100.0,
                    "filled_qty": 100.0,
                    "avg_price": 75.50,
                    "status": "filled",
                    "exec_type": "2",
                    "fix_message": "8=FIX.4.4|9=120|35=8|37=f1000000-0000-0000-0000-000000000001|11=ORD-12345|17=e1000000-0000-0000-0000-000000000001|150=2|39=2|55=AAPL|54=1|38=100|32=100|31=75.50|6=75.50|151=0|14=100|10=000|",
                    "created_at": "2026-08-31T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/fix/orders" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            status_filter = request.url.params.get("status")
            limit = int(request.url.params.get("limit", 20))
            offset = int(request.url.params.get("offset", 0))

            orders = [
                {
                    "order_id": "f1000000-0000-0000-0000-000000000001",
                    "cl_ord_id": "ORD-12345",
                    "symbol": "AAPL",
                    "side": "1",
                    "order_type": "1",
                    "qty": 100.0,
                    "filled_qty": 100.0,
                    "avg_price": 75.50,
                    "status": "filled",
                    "created_at": "2026-08-31T12:00:00Z",
                },
                {
                    "order_id": "f1000000-0000-0000-0000-000000000002",
                    "cl_ord_id": "ORD-12346",
                    "symbol": "MSFT",
                    "side": "2",
                    "order_type": "2",
                    "qty": 50.0,
                    "filled_qty": 0.0,
                    "avg_price": None,
                    "status": "open",
                    "created_at": "2026-08-31T12:05:00Z",
                },
            ]

            filtered = [o for o in orders if status_filter is None or o["status"] == status_filter.lower()]
            paged = filtered[offset : offset + limit]

            return httpx.Response(
                200,
                json={
                    "orders": paged,
                    "total": len(filtered),
                    "limit": limit,
                    "offset": offset,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/fix/cancel" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            body = json.loads(request.content.decode("utf-8")) if request.content else {}
            msg = body.get("fix_message", "")
            if not msg or "35=F" not in msg:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid FIX OrderCancelRequest message"})

            return httpx.Response(
                200,
                json={
                    "order_id": "f1000000-0000-0000-0000-000000000002",
                    "exec_id": "e1000000-0000-0000-0000-000000000002",
                    "cl_ord_id": "ORD-CANCEL-01",
                    "symbol": "MSFT",
                    "side": "2",
                    "order_type": "2",
                    "qty": 50.0,
                    "filled_qty": 0.0,
                    "avg_price": None,
                    "status": "cancelled",
                    "exec_type": "4",
                    "fix_message": "8=FIX.4.4|9=120|35=8|37=f1000000-0000-0000-0000-000000000002|11=ORD-CANCEL-01|41=ORD-12346|17=e1000000-0000-0000-0000-000000000002|150=4|39=4|55=MSFT|54=2|38=50|32=0|31=0|6=0|151=0|14=0|10=000|",
                    "created_at": "2026-08-31T12:10:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Dead Letter Queue (DLQ) Monitoring routes ───────────────────────
        if url_path == "/dlq/events" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            source_filter = request.url.params.get("source")
            status_filter = request.url.params.get("status", "failed")
            limit = int(request.url.params.get("limit", 20))
            offset = int(request.url.params.get("offset", 0))

            events = [
                {
                    "id": "550e8400-e29b-41d4-a716-446655440101",
                    "event_id": "evt_sent_fail_001",
                    "source": "sentiment",
                    "error_type": "TimeoutError",
                    "error_message": "Kafka pub-sub timeout after 5000ms",
                    "payload_preview": '{"ticker":"AAPL","sentiment_score":0.82}',
                    "failed_at": "2026-08-31T10:00:00Z",
                    "retry_count": 3,
                    "status": "failed",
                    "created_at": "2026-08-31T10:00:00Z",
                    "updated_at": "2026-08-31T10:00:00Z",
                },
                {
                    "id": "550e8400-e29b-41d4-a716-446655440102",
                    "event_id": "evt_ingest_fail_002",
                    "source": "ingestion",
                    "error_type": "ValidationError",
                    "error_message": "Malformed ISO date header",
                    "payload_preview": '{"source":"sec_edgar","filing_type":"8-K"}',
                    "failed_at": "2026-08-31T10:05:00Z",
                    "retry_count": 1,
                    "status": "failed",
                    "created_at": "2026-08-31T10:05:00Z",
                    "updated_at": "2026-08-31T10:05:00Z",
                },
            ]

            filtered = events
            if source_filter:
                filtered = [e for e in filtered if e["source"] == source_filter]
            if status_filter and status_filter != "all":
                filtered = [e for e in filtered if e["status"] == status_filter]

            paged = filtered[offset : offset + limit]
            return httpx.Response(
                200,
                json={
                    "events": paged,
                    "total": len(filtered),
                    "limit": limit,
                    "offset": offset,
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        dlq_reprocess_match = _re.match(r"^/dlq/events/([^/]+)/reprocess$", url_path)
        if dlq_reprocess_match and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            event_id = dlq_reprocess_match.group(1)
            if event_id == "550e8400-e29b-41d4-a716-446655440999":
                return httpx.Response(404, json={"error": "Not Found", "message": "Event not found"})

            return httpx.Response(
                200,
                json={
                    "id": event_id,
                    "event_id": "evt_sent_fail_001",
                    "status": "reprocessed",
                    "retry_count": 4,
                    "message": "Event reprocessed successfully and republished to processing pipeline",
                    "reprocessed_at": "2026-08-31T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        dlq_get_match = _re.match(r"^/dlq/events/([^/]+)$", url_path)
        if dlq_get_match and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            event_id = dlq_get_match.group(1)
            if event_id == "550e8400-e29b-41d4-a716-446655440999":
                return httpx.Response(404, json={"error": "Not Found", "message": "Event not found"})

            return httpx.Response(
                200,
                json={
                    "id": event_id,
                    "event_id": "evt_sent_fail_001",
                    "source": "sentiment",
                    "error_type": "TimeoutError",
                    "error_message": "Kafka pub-sub timeout after 5000ms",
                    "payload": {"ticker": "AAPL", "sentiment_score": 0.82, "raw_text": "Sample text for Apple earnings beat."},
                    "failed_at": "2026-08-31T10:00:00Z",
                    "retry_count": 3,
                    "status": "failed",
                    "created_at": "2026-08-31T10:00:00Z",
                    "updated_at": "2026-08-31T10:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if dlq_get_match and method == "DELETE":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            event_id = dlq_get_match.group(1)
            if event_id == "550e8400-e29b-41d4-a716-446655440999":
                return httpx.Response(404, json={"error": "Not Found", "message": "Event not found"})

            return httpx.Response(
                200,
                json={
                    "id": event_id,
                    "event_id": "evt_sent_fail_001",
                    "status": "purged",
                    "message": "Event purged from quarantine and marked as purged",
                    "purged_at": "2026-08-31T12:05:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Latency SLA Status & Reporting route ─────────────────────────────
        if url_path == "/sla/status" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            start_date = request.url.params.get("start_date", "2025-08-01")
            end_date = request.url.params.get("end_date", "2025-08-31")
            percentiles_param = request.url.params.get("percentiles", "50,95,99")
            sla_target_ms = int(request.url.params.get("sla_target_ms", "100"))

            percentiles_map = {}
            for p_str in percentiles_param.split(","):
                cleaned = p_str.strip().lstrip("pP")
                if cleaned:
                    p_val = float(cleaned)
                    key = f"p{int(p_val)}" if p_val.is_integer() else f"p{p_val:.1f}"
                    if p_val == 50:
                        percentiles_map[key] = 4.85
                    elif p_val == 95:
                        percentiles_map[key] = 14.20
                    elif p_val == 99:
                        percentiles_map[key] = 28.50
                    else:
                        percentiles_map[key] = round(p_val * 0.35, 2)

            compliance_rate = 99.9 if sla_target_ms >= 50 else (92.5 if sla_target_ms >= 10 else 50.0)
            status_val = "met" if compliance_rate >= 99.9 else "breached"

            return httpx.Response(
                200,
                json={
                    "start_date": start_date,
                    "end_date": end_date,
                    "total_requests": 3000,
                    "average_latency_ms": 5.25,
                    "percentiles": percentiles_map,
                    "sla_target_ms": sla_target_ms,
                    "compliant_requests": int(3000 * compliance_rate / 100.0),
                    "sla_compliance_rate": compliance_rate,
                    "sla_status": status_val,
                    "generated_at": "2026-08-31T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── API Sandbox Environment routes ─────────────────────────────
        if url_path == "/sandbox/activate" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "active": True,
                    "mock_data_version": "sandbox-v1.0",
                    "activated_at": "2026-09-01T10:00:00Z",
                    "deactivated_at": None,
                    "available_endpoints": ["/sentiment", "/sentiment/feed", "/options/iv", "/sla/status"],
                    "message": "Sandbox mode is active. Requests will be served with isolated mock data and will not consume production quota.",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60", "X-FinText-Sandbox": "true"},
            )

        if url_path == "/sandbox/deactivate" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "active": False,
                    "mock_data_version": "sandbox-v1.0",
                    "activated_at": "2026-09-01T10:00:00Z",
                    "deactivated_at": "2026-09-01T10:15:00Z",
                    "available_endpoints": ["/sentiment", "/sentiment/feed", "/options/iv", "/sla/status"],
                    "message": "Sandbox mode is inactive. Requests will be served with live production data stores.",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        if url_path == "/sandbox/status" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "active": True,
                    "mock_data_version": "sandbox-v1.0",
                    "activated_at": "2026-09-01T10:00:00Z",
                    "deactivated_at": None,
                    "available_endpoints": ["/sentiment", "/sentiment/feed", "/options/iv", "/sla/status"],
                    "message": "Sandbox mode is active. Requests will be served with isolated mock data and will not consume production quota.",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60", "X-FinText-Sandbox": "true"},
            )

        # ─── Data Provenance Lineage routes ─────────────────────────────
        if url_path.startswith("/provenance/") and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            parts = url_path.strip("/").split("/")
            if len(parts) < 3:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid provenance URL"})

            rec_type = parts[1]
            rec_id = parts[2]

            if rec_type not in ("sentiment", "news"):
                return httpx.Response(400, json={"error": "Bad Request", "message": f"Invalid record_type '{rec_type}'"})

            return httpx.Response(
                200,
                json={
                    "record_type": rec_type,
                    "record_id": rec_id,
                    "provenance_entries": [
                        {
                            "id": "550e8400-e29b-41d4-a716-446655440000",
                            "record_type": rec_type,
                            "record_id": rec_id,
                            "source_type": "finnhub" if rec_type == "sentiment" else "sec_edgar",
                            "source_id": "art_123456",
                            "model_version": "finbert-v3.1.0",
                            "pipeline_version": "2.0.0",
                            "data_quality_score": 0.98,
                            "processing_steps": [
                                {
                                    "step": "fetch_article" if rec_type == "sentiment" else "fetch_source",
                                    "timestamp": "2026-08-30T10:15:00.100Z",
                                    "description": "Ingested raw payload from wire provider",
                                    "details": {"http_status": 200},
                                },
                                {
                                    "step": "clean_text" if rec_type == "sentiment" else "deduplication",
                                    "timestamp": "2026-08-30T10:15:00.200Z",
                                    "description": "Normalized text and stripped boilerplate",
                                    "details": None,
                                },
                                {
                                    "step": "tokenize" if rec_type == "sentiment" else "entity_extraction",
                                    "timestamp": "2026-08-30T10:15:00.300Z",
                                    "description": "Tokenized financial lexicon",
                                    "details": {"token_count": 342},
                                },
                                {
                                    "step": "infer_sentiment" if rec_type == "sentiment" else "sentiment_tagging",
                                    "timestamp": "2026-08-30T10:15:00.500Z",
                                    "description": "Executed forward pass on transformer model",
                                    "details": {"inference_latency_ms": 1.42},
                                },
                                {
                                    "step": "quality_audit" if rec_type == "sentiment" else "publish_index",
                                    "timestamp": "2026-08-30T10:15:00.600Z",
                                    "description": "Validated quality invariants",
                                    "details": {"quality_score": 0.98},
                                },
                            ],
                            "created_at": "2026-08-30T10:15:01.000Z",
                        }
                    ],
                    "total_entries": 1,
                    "retrieved_at": "2026-09-01T12:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # ─── Sentiment Anomaly Scan Trigger route ─────────────────────────
        if url_path == "/anomaly-scan" and method == "POST":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            return httpx.Response(
                200,
                json={
                    "anomalies_found": 2,
                    "alerts_broadcasted": 2,
                    "anomalies": [
                        {
                            "type": "sentiment_anomaly",
                            "ticker": "AAPL",
                            "latest_score": 0.85,
                            "mean_score": 0.12,
                            "stddev": 0.21,
                            "zscore": 3.48,
                            "direction": "bullish",
                            "timestamp": "2026-09-01T12:00:00Z",
                        },
                        {
                            "type": "sentiment_anomaly",
                            "ticker": "NVDA",
                            "latest_score": -0.72,
                            "mean_score": 0.05,
                            "stddev": 0.24,
                            "zscore": -3.21,
                            "direction": "bearish",
                            "timestamp": "2026-09-01T12:00:00Z",
                        },
                    ],
                    "scanned_at": "2026-09-01T12:00:00Z",
                    "message": "Anomaly scan completed: 2 anomalies detected and broadcast to 1 active WebSocket receivers",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        # Provider Health Status: GET /providers/health
        if url_path == "/providers/health" and method == "GET":
            auth = request.headers.get("Authorization", "")
            if not auth.startswith("Bearer "):
                return httpx.Response(401, json={"error": "Unauthorized", "message": "Missing Bearer token"})

            provider_param = request.url.params.get("provider", "all").lower()
            window_str = request.url.params.get("window_minutes", "60")
            try:
                window_val = int(window_str)
                if not (1 <= window_val <= 1440):
                    return httpx.Response(400, json={"error": "Bad Request", "message": "window_minutes must be between 1 and 1440"})
            except ValueError:
                return httpx.Response(400, json={"error": "Bad Request", "message": "Invalid window_minutes"})

            if provider_param not in ("sec_edgar", "finnhub", "polygon", "all"):
                return httpx.Response(400, json={"error": "Bad Request", "message": f"Unsupported provider '{provider_param}'"})

            providers = []
            if provider_param in ("all", "sec_edgar"):
                providers.append({
                    "provider": "sec_edgar",
                    "status": "healthy",
                    "requests_total": 120,
                    "requests_success": 118,
                    "success_rate_pct": 98.33,
                    "avg_latency_ms": 250.5,
                    "p95_latency_ms": 480.0,
                    "error_count_last_hour": 2,
                    "last_success_timestamp": "2026-09-05T10:30:00Z",
                    "last_error_message": "Timeout after 5000ms",
                })
            if provider_param in ("all", "finnhub"):
                providers.append({
                    "provider": "finnhub",
                    "status": "healthy",
                    "requests_total": 450,
                    "requests_success": 448,
                    "success_rate_pct": 99.56,
                    "avg_latency_ms": 45.2,
                    "p95_latency_ms": 110.0,
                    "error_count_last_hour": 2,
                    "last_success_timestamp": "2026-09-05T10:31:00Z",
                    "last_error_message": None,
                })
            if provider_param in ("all", "polygon"):
                providers.append({
                    "provider": "polygon",
                    "status": "degraded",
                    "requests_total": 800,
                    "requests_success": 720,
                    "success_rate_pct": 90.0,
                    "avg_latency_ms": 85.0,
                    "p95_latency_ms": 220.0,
                    "error_count_last_hour": 80,
                    "last_success_timestamp": "2026-09-05T10:29:00Z",
                    "last_error_message": "Rate limit 429 exceeded",
                })

            return httpx.Response(
                200,
                json={
                    "providers": providers,
                    "generated_at": "2026-09-05T11:00:00Z",
                },
                headers={"X-RateLimit-Limit": "100", "X-RateLimit-Remaining": "99", "X-RateLimit-Reset": "60"},
            )

        return httpx.Response(404, json={"error": "Not Found", "message": f"Route {url_path} not found"})

    return httpx.MockTransport(handler)


def test_create_retraining_job():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.create_retraining_job(
        model_type="sentiment",
        trigger_type="manual",
        config={"epochs": 3, "batch_size": 32, "learning_rate": 2e-5},
    )

    assert isinstance(resp, RetrainingJobResponse)
    assert isinstance(resp.job, RetrainingJob)
    assert resp.job.model_type == "sentiment"
    assert resp.job.trigger_type == "manual"
    assert resp.job.status == "pending"
    assert resp.job.config["epochs"] == 3
    assert resp.job.config["batch_size"] == 32
    assert resp.job.config["learning_rate"] == 2e-5
    assert resp.message == "Retraining job queued successfully"


def test_list_retraining_jobs():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.list_retraining_jobs(status="completed", limit=20)

    assert isinstance(resp, ListRetrainingJobsResponse)
    assert len(resp.jobs) == 1
    assert resp.total == 1
    assert resp.jobs[0].status == "completed"
    assert resp.jobs[0].config.get("metrics") is not None
    assert resp.jobs[0].config["metrics"]["accuracy"] == 0.952


def test_get_and_cancel_retraining_job():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # Get job
    resp = client.get_retraining_job("11111111-2222-3333-4444-555555555555")
    assert isinstance(resp, RetrainingJobResponse)
    assert resp.job.id == "11111111-2222-3333-4444-555555555555"
    assert resp.job.status == "completed"

    # Cancel job
    cancel_resp = client.cancel_retraining_job("11111111-2222-3333-4444-555555555555")
    assert isinstance(cancel_resp, RetrainingJobResponse)
    assert cancel_resp.job.status == "cancelled"
    assert cancel_resp.message == "Retraining job cancelled successfully"


def test_health_check():
    client = FinTextClient(base_url="http://mock.fintext", transport=create_mock_transport())
    health = client.health()

    assert isinstance(health, HealthResponse)
    assert health.status == "ok"
    assert health.version == "2.0.0-institutional"
    assert health.timestamp_us > 0


def test_token_issuance_and_manual_credential_setup():
    client = FinTextClient(
        base_url="http://mock.fintext",
        admin_token="valid_admin_token",
        transport=create_mock_transport(),
    )
    token = client.get_token(user_id="test_quant_fund", expires_in_seconds=7200)

    assert token == "jwt_mock_token_for_test_quant_fund"
    assert client.api_token == token

    # Test manual token override
    client.set_token("manual_override_token")
    assert client.api_token == "manual_override_token"


def test_auto_authentication_flow():
    client = FinTextClient(
        base_url="http://mock.fintext",
        admin_token="valid_admin_token",
        auto_auth_user="auto_quant",
        transport=create_mock_transport(),
    )
    assert client.api_token is None

    # First protected call should automatically acquire token using admin_token
    sentiment = client.sentiment("AAPL")

    assert client.api_token == "jwt_mock_token_for_auto_quant"
    assert isinstance(sentiment, SentimentResponse)
    assert sentiment.ticker == "AAPL"
    assert sentiment.sentiment_score == 0.45
    assert sentiment.sentiment_label == "BULLISH"
    assert sentiment.confidence == 0.88
    assert sentiment.probabilities is not None
    assert sentiment.probabilities.positive == 0.88

    # Verify rate limit header tracking
    assert client.last_rate_limit.limit == 100
    assert client.last_rate_limit.remaining == 98
    assert client.last_rate_limit.reset == 59


def test_spillovers_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    spillovers = client.spillovers("AAPL", limit=5)

    assert isinstance(spillovers, SpilloverResponse)
    assert spillovers.ticker == "AAPL"
    assert spillovers.count == 1
    assert spillovers.spillovers[0].related_ticker == "MSFT"
    assert spillovers.spillovers[0].correlation == 0.745


def test_backtest_simulation():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    req = BacktestRequest(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
        long_threshold=0.2,
        short_threshold=-0.2,
        holding_days=5,
        initial_capital=1000000.0,
    )
    result = client.backtest(req)

    assert result.ticker == "AAPL"
    assert result.total_return == 0.185
    assert result.sharpe_ratio == 2.45
    assert len(result.equity_curve) == 3


def test_backtest_with_dict_payload():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    result = client.backtest({
        "ticker": "NVDA",
        "start_date": "2025-01-01",
        "end_date": "2025-03-31",
    })
    assert result.ticker == "NVDA"
    assert result.total_return == 0.185


def test_backtest_multi_asset_portfolio():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    req = BacktestRequest(
        tickers=["AAPL", "NVDA", "MSFT"],
        weights=[0.5, 0.3, 0.2],
        benchmark_ticker="SPY",
        transaction_cost_bps=10.0,
        start_date="2025-01-01",
        end_date="2025-03-31",
        long_threshold=0.2,
        short_threshold=-0.2,
        holding_days=5,
        initial_capital=1000000.0,
    )
    result = client.backtest(req)

    assert result.tickers == ["AAPL", "NVDA", "MSFT"]
    assert result.weights == [0.5, 0.3, 0.2]
    assert result.benchmark_ticker == "SPY"
    assert result.transaction_cost_bps == 10.0
    assert result.sortino_ratio == 3.82
    assert result.profit_factor == 2.14
    assert len(result.equity_points) == 3
    assert result.equity_points[0].portfolio_value == 1000000.0
    assert result.alpha == 0.160


def test_ws_url_generation():
    client = FinTextClient(
        base_url="http://localhost:8000",
        api_token="mock_ws_jwt_token",
    )
    url = client.ws_url(ticker="AAPL")
    assert url == "ws://localhost:8000/ws?token=mock_ws_jwt_token&ticker=AAPL"

    # HTTPS conversion to WSS
    client_https = FinTextClient(
        base_url="https://api.fintext.alpha",
        api_token="secure_token",
    )
    url_https = client_https.ws_url()
    assert url_https == "wss://api.fintext.alpha/ws?token=secure_token"


def test_unauthenticated_call_raises_error():
    client = FinTextClient(
        base_url="http://mock.fintext",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextAuthError) as exc_info:
        client.sentiment("AAPL")

    assert "Authentication required" in str(exc_info.value)


def test_rate_limit_exception_handling():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="rate_limited_token",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextRateLimitError) as exc_info:
        client.sentiment("AAPL")

    err = exc_info.value
    assert err.status_code == 429
    assert err.retry_after == 15
    assert err.limit == 100
    assert err.remaining == 0
    assert err.reset == 15


def test_parameter_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_token",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextValidationError):
        client.sentiment("")

    with pytest.raises(FinTextValidationError):
        client.spillovers("   ")

    with pytest.raises(FinTextValidationError):
        client.get_token(user_id="")


def test_context_manager():
    with FinTextClient(base_url="http://mock.fintext", transport=create_mock_transport()) as client:
        health = client.health()
        assert health.status == "ok"


def test_sentiment_history_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    history = client.sentiment_history(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-01-10",
        limit=2,
        offset=0,
        sort="asc",
    )
    assert history.ticker == "AAPL"
    assert history.count == 2
    assert history.total == 10
    assert len(history.records) == 2
    assert history.records[0].sentiment_score == 0.25
    assert history.records[1].gamma_exposure == 59940.0


def test_spillover_matrix_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    matrix_resp = client.spillover_matrix(
        start_date="2025-01-01",
        end_date="2025-03-31",
        tickers=["AAPL", "MSFT", "NVDA"],
        min_correlation=0.5,
        max_lag_hours=24,
    )
    assert matrix_resp.tickers == ["AAPL", "MSFT", "NVDA"]
    assert matrix_resp.min_correlation == 0.5
    assert matrix_resp.count == 3
    assert matrix_resp.matrix[0].ticker_a == "AAPL"
    assert matrix_resp.matrix[0].ticker_b == "MSFT"
    assert matrix_resp.matrix[0].correlation == 0.75
    assert matrix_resp.matrix[0].direction == "AAPL_leads_MSFT"


def test_sector_sentiment_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    sec_resp = client.sector_sentiment(
        sector="Technology",
        start_date="2025-01-01",
        end_date="2025-03-31",
        aggregation="average",
        min_confidence=0.5,
    )
    assert sec_resp.sector == "Technology"
    assert sec_resp.aggregation == "average"
    assert sec_resp.min_confidence == 0.5
    assert sec_resp.value == 0.42
    assert sec_resp.record_count == 150
    assert sec_resp.tickers_included == 22
    assert client.last_rate_limit.remaining == 96


def test_batch_sentiment_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    batch_resp = client.batch_sentiment(
        tickers=["AAPL", "MSFT", "NVDA"],
        date="2025-01-15",
    )
    assert batch_resp.count == 3
    assert len(batch_resp.results) == 3
    assert batch_resp.results[0].ticker == "AAPL"
    assert batch_resp.results[0].sentiment_score == 0.50
    assert batch_resp.results[0].confidence == 0.85
    assert batch_resp.results[1].ticker == "MSFT"
    assert batch_resp.results[2].ticker == "NVDA"
    assert client.last_rate_limit.remaining == 95


def test_symbol_map_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    # Test Auto -> All
    res_all = client.symbol_map(identifier="AAPL", output_type="all")
    assert res_all.input_identifier == "AAPL"
    assert res_all.input_type == "ticker"
    assert res_all.result.ticker == "AAPL"
    assert res_all.result.figi == "BBG000B9XRY4"
    assert res_all.result.cusip == "037833100"
    assert res_all.result.isin == "US0378331005"
    assert client.last_rate_limit.remaining == 94

    # Test ISIN -> Ticker
    res_ticker = client.symbol_map(identifier="US0378331005", input_type="isin", output_type="ticker")
    assert res_ticker.input_type == "isin"
    assert res_ticker.output_type == "ticker"
    assert res_ticker.result.ticker == "AAPL"
    assert res_ticker.result.figi is None


def test_universe_crud():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    # 1. Create Universe
    u = client.create_universe(name="Tech Watchlist", tickers=["AAPL", "MSFT"])
    assert u.id == "u-123"
    assert u.name == "Tech Watchlist"
    assert u.tickers == ["AAPL", "MSFT"]

    # 2. List Universes
    list_res = client.list_universes()
    assert list_res.count == 1
    assert list_res.universes[0].id == "u-123"

    # 3. Get Universe
    get_res = client.get_universe(universe_id="u-123")
    assert get_res.id == "u-123"

    # 4. Update Universe
    up_res = client.update_universe(universe_id="u-123", name="Updated Watchlist", tickers=["AAPL", "MSFT", "NVDA"])
    assert up_res.name == "Updated Watchlist"
    assert up_res.tickers == ["AAPL", "MSFT", "NVDA"]

    # 5. Delete Universe
    del_res = client.delete_universe(universe_id="u-123")
    assert del_res.id == "u-123"
    assert del_res.status == "deleted"


def test_batch_sentiment_universe():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    batch_resp = client.batch_sentiment(universe_id="u-123")
    assert batch_resp.count == 2
    assert batch_resp.results[0].ticker == "AAPL"
    assert batch_resp.results[1].ticker == "MSFT"


def test_sentiment_history_min_quality_and_scores():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.sentiment_history(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-01-10",
        min_quality=0.85,
    )
    assert resp.count == 2
    assert resp.records[0].data_quality_score == 0.88


def test_export_csv():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    csv_text = client.export_csv(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-01-10",
        min_quality=0.80,
    )
    assert "data_quality_score" in csv_text
    assert "AAPL" in csv_text


def test_options_iv_chain():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.options_iv(
        ticker="AAPL",
        expiration_date="2025-12-19",
        option_type="all",
    )
    assert resp.ticker == "AAPL"
    assert resp.expiration_date == "2025-12-19"
    assert resp.underlying_price == 224.50
    assert resp.count == 2
    assert len(resp.contracts) == 2
    assert resp.contracts[0].option_type == "CALL"
    assert resp.contracts[0].delta > 0
    assert resp.contracts[1].option_type == "PUT"
    assert resp.contracts[1].delta < 0


def test_options_iv_single_strike():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.options_iv(
        ticker="AAPL",
        expiration_date="2025-12-19",
        option_type="call",
        strike=250.0,
    )
    assert resp.ticker == "AAPL"
    assert resp.count == 1
    assert resp.contracts[0].strike == 250.0
    assert resp.contracts[0].implied_volatility == 0.2850
    assert resp.contracts[0].delta == 0.4215
    assert resp.contracts[0].gamma == 0.0098


def test_unusual_options_all_tickers():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.unusual_options(
        min_volume_oi_ratio=2.0,
        min_volume=100,
        days=1,
        limit=10,
    )
    assert resp.ticker == "ALL"
    assert resp.count == 2
    assert len(resp.items) == 2
    assert resp.items[0].ticker == "O:AAPL251219C00250000"
    assert resp.items[0].underlying_ticker == "AAPL"
    assert resp.items[0].volume == 48500
    assert resp.items[0].volume_oi_ratio > 20.0
    assert resp.items[0].score > 0.0
    assert resp.items[1].underlying_ticker == "NVDA"


def test_unusual_options_with_ticker_filter():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.unusual_options(ticker="AAPL")
    assert resp.ticker == "AAPL"
    assert resp.count == 1
    assert resp.items[0].underlying_ticker == "AAPL"


def test_unusual_options_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.unusual_options(min_volume_oi_ratio=-1.0)

    with pytest.raises(FinTextValidationError):
        client.unusual_options(days=0)

    with pytest.raises(FinTextValidationError):
        client.unusual_options(days=10)

    with pytest.raises(FinTextValidationError):
        client.unusual_options(limit=0)


def test_usage_stats_default_parameters():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.usage_stats()
    assert resp.user_id == "test_quant_fund"
    assert resp.group_by == "day"
    assert resp.summary.total_requests == 1250
    assert resp.summary.successful_requests == 1205
    assert resp.summary.failed_requests == 45
    assert resp.summary.rate_limited_requests == 12
    assert resp.summary.average_latency_ms == 4.85
    assert resp.summary.p95_latency_ms == 14.20
    assert resp.summary.max_latency_ms == 45.60
    assert len(resp.breakdown) == 2
    assert resp.breakdown[0].key == "2026-08-28"


def test_usage_stats_endpoint_grouping():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.usage_stats(
        start_date="2026-08-01",
        end_date="2026-08-29",
        group_by="endpoint",
        limit=10,
    )
    assert resp.group_by == "endpoint"
    assert len(resp.breakdown) == 2
    assert resp.breakdown[0].key == "/sentiment"
    assert resp.breakdown[0].count == 560
    assert resp.breakdown[1].key == "/options/iv"
    assert resp.breakdown[1].count == 250


def test_usage_stats_status_code_grouping():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.usage_stats(group_by="status_code")
    assert resp.group_by == "status_code"
    assert len(resp.breakdown) == 3
    assert resp.breakdown[0].key == "200"
    assert resp.breakdown[0].successful_requests == 1205
    assert resp.breakdown[1].key == "400"
    assert resp.breakdown[2].key == "429"


def test_usage_stats_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.usage_stats(group_by="invalid_group")

    with pytest.raises(FinTextValidationError):
        client.usage_stats(limit=0)

    with pytest.raises(FinTextValidationError):
        client.usage_stats(limit=1001)


def test_event_study_success():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.event_study(
        ticker="AAPL",
        event_date="2025-06-15",
        event_window=5,
        estimation_window=60,
        benchmark_ticker="SPY",
    )
    assert resp.ticker == "AAPL"
    assert resp.event_date == "2025-06-15"
    assert resp.event_window == 5
    assert resp.estimation_window == 60
    assert resp.benchmark_ticker == "SPY"
    assert resp.count == 11
    assert len(resp.abnormal_returns) == 11
    assert resp.abnormal_returns[5].day_offset == 0
    assert resp.alpha == 0.0005
    assert resp.beta == 1.20


def test_event_study_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="", event_date="2025-06-15")

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="AAPL", event_date="")

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="AAPL", event_date="2025-06-15", event_window=0)

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="AAPL", event_date="2025-06-15", event_window=25)

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="AAPL", event_date="2025-06-15", estimation_window=5)

    with pytest.raises(FinTextValidationError):
        client.event_study(ticker="AAPL", event_date="2025-06-15", estimation_window=150)


def test_recent_8k_success():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    # 1. All filings
    resp = client.recent_8k(days=7, limit=10)
    assert resp.ticker == "ALL"
    assert resp.event_type == "ALL"
    assert resp.days == 7
    assert resp.count == 3
    assert len(resp.filings) == 3
    assert resp.filings[0].ticker == "AAPL"
    assert resp.filings[0].event_type == "M&A"
    assert resp.filings[0].form_type == "8-K"
    assert "1.01" in resp.filings[0].items

    # 2. Filter by ticker
    resp_aapl = client.recent_8k(ticker="AAPL")
    assert resp_aapl.ticker == "AAPL"
    assert resp_aapl.count == 2
    assert all(f.ticker == "AAPL" for f in resp_aapl.filings)

    # 3. Filter by event_type
    resp_ceo = client.recent_8k(event_type="CEO Change")
    assert resp_ceo.event_type == "CEO Change"
    assert resp_ceo.count == 1
    assert resp_ceo.filings[0].ticker == "NVDA"


def test_recent_8k_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.recent_8k(days=0)

    with pytest.raises(FinTextValidationError):
        client.recent_8k(days=35)

    with pytest.raises(FinTextValidationError):
        client.recent_8k(limit=0)

    with pytest.raises(FinTextValidationError):
        client.recent_8k(limit=600)

    with pytest.raises(FinTextValidationError):
        client.recent_8k(ticker="   ")

    with pytest.raises(FinTextValidationError):
        client.recent_8k(event_type="   ")


def test_sentiment_feed_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_feed()
    assert resp.count == 2
    assert resp.total == 50
    assert resp.limit == 100
    assert resp.offset == 0
    assert resp.sort == "desc"
    assert len(resp.records) == 2
    assert resp.records[0].ticker == "AAPL"
    assert resp.records[0].sentiment_score == 0.45
    assert resp.records[0].sentiment_label == "BULLISH"
    assert resp.records[0].confidence == 0.88
    assert resp.records[0].data_quality_score == 0.92
    assert resp.records[0].vpin == 0.42
    assert resp.records[0].gamma_exposure == 150000.0


def test_sentiment_feed_with_filters():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_feed(
        sector="Technology",
        start_date="2026-08-28T00:00:00Z",
        end_date="2026-08-29T00:00:00Z",
        min_confidence=0.7,
        min_quality=0.8,
        limit=50,
        offset=10,
        sort="asc",
    )
    assert resp.sector == "Technology"
    assert resp.min_confidence == 0.7
    assert resp.min_quality == 0.8
    assert resp.limit == 50
    assert resp.offset == 10
    assert resp.sort == "asc"
    assert len(resp.records) == 2


def test_sentiment_feed_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(sort="invalid_sort")

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(min_confidence=-0.1)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(min_confidence=1.5)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(min_quality=-0.5)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(min_quality=2.0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(limit=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(limit=1001)

    with pytest.raises(FinTextValidationError):
        client.sentiment_feed(offset=-1)


def test_sentiment_feed_sector_not_found():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextAPIError) as exc_info:
        client.sentiment_feed(sector="InvalidSector")
    assert exc_info.value.status_code == 404


def test_sentiment_feed_cursor():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    # 1. First page returns next_cursor
    resp1 = client.sentiment_feed(limit=2)
    assert resp1.count == 2
    assert resp1.next_cursor is not None
    assert resp1.next_cursor == "2026-08-29T14:15:00.000000Z"

    # 2. Query page 2 with cursor
    resp2 = client.sentiment_feed(cursor=resp1.next_cursor, limit=2)
    assert resp2.count == 2
    assert resp2.next_cursor is not None


def test_sentiment_feed_cursor_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    # Mutual exclusion: cannot specify both cursor and offset
    with pytest.raises(FinTextValidationError, match="Cannot specify both 'cursor' and 'offset'"):
        client.sentiment_feed(cursor="2026-08-29T14:30:00Z", offset=10)

    # Empty cursor rejected
    with pytest.raises(FinTextValidationError, match="cursor cannot be empty"):
        client.sentiment_feed(cursor="   ")


def test_sentiment_anomalies_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_anomalies()
    assert isinstance(resp, SentimentAnomaliesResponse)
    assert resp.count == 2
    assert resp.total_anomalies_detected == 2
    assert resp.lookback_days == 30
    assert resp.zscore_threshold == 2.0
    assert resp.min_records == 20
    assert resp.scanned_tickers == 12
    assert len(resp.items) == 2
    assert resp.items[0].ticker == "AAPL"
    assert resp.items[0].direction == "bullish"
    assert resp.items[0].zscore == 3.0
    assert resp.items[1].ticker == "MSFT"
    assert resp.items[1].direction == "bearish"
    assert resp.items[1].zscore == -3.2


def test_sentiment_anomalies_with_filters():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_anomalies(
        sector="Technology",
        lookback_days=45,
        zscore_threshold=2.5,
        min_records=30,
        limit=10,
    )
    assert isinstance(resp, SentimentAnomaliesResponse)
    assert resp.sector == "Technology"
    assert resp.lookback_days == 45
    assert resp.zscore_threshold == 2.5
    assert resp.min_records == 30


def test_sentiment_anomalies_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(lookback_days=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(lookback_days=91)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(zscore_threshold=0.5)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(zscore_threshold=6.0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(min_records=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(min_records=1001)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(limit=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_anomalies(limit=101)


def test_sentiment_anomalies_sector_not_found():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextAPIError) as exc_info:
        client.sentiment_anomalies(sector="InvalidSector")
    assert exc_info.value.status_code == 404


def test_transcribe_audio_bytes_success():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    fake_wav = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x80>\x00\x00\x00}\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00"
    resp = client.transcribe_audio(fake_wav, filename="call.wav")
    assert isinstance(resp, AudioTranscriptionResponse)
    assert resp.language == "en"
    assert resp.duration_seconds == 120.5
    assert resp.sentiment.label == "BULLISH"
    assert resp.sentiment.score == 0.725
    assert resp.acoustic_features.pitch_mean_hz == 120.3
    assert resp.acoustic_features.energy_rms == 0.05
    assert resp.acoustic_features.pause_ratio == 0.15


def test_transcribe_audio_validation_and_errors(tmp_path):
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    # 1. Non-existent file path
    with pytest.raises(FinTextValidationError):
        client.transcribe_audio("non_existent_audio_file_999.wav")

    # 2. Unsupported audio type
    with pytest.raises(FinTextValidationError):
        client.transcribe_audio(12345)  # type: ignore

    # 3. File size exceeded (>50 MB)
    large_bytes = b"0" * (50 * 1024 * 1024 + 1)
    with pytest.raises(FinTextValidationError):
        client.transcribe_audio(large_bytes, filename="huge.wav")

    # 4. Invalid file extension rejected by API
    test_txt = tmp_path / "invalid.txt"
    test_txt.write_text("not audio")
    with pytest.raises(FinTextAPIError) as exc_info:
        client.transcribe_audio(test_txt)
    assert exc_info.value.status_code == 400


def test_transcribe_audio_with_store_option():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    fake_wav = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x80>\x00\x00\x00}\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00"
    resp = client.transcribe_audio(fake_wav, filename="call.wav", store=True, ticker="AAPL", quarter=4, year=2024)
    assert resp.transcript_id == "t-12345-uuid"


def test_transcript_crud_lifecycle():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    # 1. Create transcript
    created = client.create_transcript(
        ticker="AAPL",
        transcript_text="Apple Inc. reported record quarterly revenue of $94.9 billion.",
        quarter=4,
        year=2024,
        call_date="2024-10-31",
    )
    assert isinstance(created, TranscriptResponse)
    assert created.ticker == "AAPL"
    assert created.id == "t-12345-uuid"
    assert created.sentiment_label == "BULLISH"

    # 2. Get transcript
    fetched = client.get_transcript(created.id)
    assert isinstance(fetched, TranscriptResponse)
    assert fetched.id == created.id
    assert "Apple Inc." in fetched.transcript_text

    # 3. List transcripts
    listing = client.list_transcripts(ticker="AAPL")
    assert isinstance(listing, TranscriptListResponse)
    assert listing.total >= 1
    assert listing.items[0].ticker == "AAPL"

    # 4. Delete transcript
    deleted = client.delete_transcript(created.id)
    assert isinstance(deleted, DeleteTranscriptResponse)
    assert deleted.id == created.id
    assert deleted.deleted is True


def test_transcript_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.create_transcript(ticker="", transcript_text="valid text here")

    with pytest.raises(FinTextValidationError):
        client.create_transcript(ticker="AAPL", transcript_text="")

    with pytest.raises(FinTextValidationError):
        client.get_transcript(transcript_id="")

    with pytest.raises(FinTextValidationError):
        client.delete_transcript(transcript_id="")


def test_market_regime_query():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    regime = client.market_regime(lookback_days=5, min_data_points=50)
    assert regime.regime == "Bullish"
    assert regime.confidence == 0.85
    assert regime.market_sentiment == 0.24
    assert regime.breadth == 0.68
    assert regime.lookback_days == 5
    assert regime.components.total_data_points == 120
    assert regime.components.sector_sentiments["Technology"] == 0.32


def test_market_regime_custom_weights():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    regime = client.market_regime(
        lookback_days=7,
        sector_weights="Technology:0.5,Financials:0.3,Healthcare:0.2",
        min_data_points=60,
    )
    assert regime.regime == "Bullish"
    assert regime.lookback_days == 7


def test_market_regime_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.market_regime(lookback_days=0)

    with pytest.raises(FinTextValidationError):
        client.market_regime(lookback_days=35)

    with pytest.raises(FinTextValidationError):
        client.market_regime(min_data_points=0)

    with pytest.raises(FinTextValidationError):
        client.market_regime(min_data_points=1005)


def test_return_correlation_matrix():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.return_correlation(
        tickers=["AAPL", "MSFT", "NVDA"],
        start_date="2025-01-01",
        end_date="2025-03-31",
        min_periods=20,
    )
    assert isinstance(resp, ReturnCorrelationResponse)
    assert resp.tickers == ["AAPL", "MSFT", "NVDA"]
    assert resp.start_date == "2025-01-01"
    assert resp.end_date == "2025-03-31"
    assert resp.min_periods == 20
    assert len(resp.matrix) == 3
    assert resp.matrix[0].ticker_a == "AAPL"
    assert resp.matrix[0].ticker_b == "MSFT"
    assert resp.matrix[0].correlation == 0.72
    assert resp.matrix[0].periods == 60


def test_return_correlation_include_self():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.return_correlation(
        tickers="AAPL,MSFT",
        start_date="2025-01-01",
        end_date="2025-03-31",
        include_self=True,
    )
    assert isinstance(resp, ReturnCorrelationResponse)
    # N=2 with include_self => (AAPL, AAPL), (AAPL, MSFT), (MSFT, MSFT) => 3 entries
    assert len(resp.matrix) == 3
    self_entry = next(i for i in resp.matrix if i.ticker_a == "AAPL" and i.ticker_b == "AAPL")
    assert self_entry.correlation == 1.0


def test_return_correlation_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.return_correlation(tickers="", start_date="2025-01-01", end_date="2025-03-31")

    with pytest.raises(FinTextValidationError):
        client.return_correlation(tickers="AAPL,MSFT", start_date="", end_date="2025-03-31")

    with pytest.raises(FinTextValidationError):
        client.return_correlation(tickers="AAPL,MSFT", start_date="2025-01-01", end_date="")

    with pytest.raises(FinTextValidationError):
        client.return_correlation(tickers="AAPL,MSFT", start_date="2025-01-01", end_date="2025-03-31", min_periods=5)

    with pytest.raises(FinTextValidationError):
        client.return_correlation(tickers="AAPL,MSFT", start_date="2025-01-01", end_date="2025-03-31", min_periods=1005)


def test_put_call_ratio_daily():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.put_call_ratio(
        start_date="2025-01-01",
        end_date="2025-01-10",
        ticker="AAPL",
        ratio_type="volume",
        granularity="daily",
    )
    assert isinstance(resp, PutCallRatioResponse)
    assert resp.ticker == "AAPL"
    assert resp.granularity == "daily"
    assert resp.ratio_type == "volume"
    assert resp.points is not None
    assert len(resp.points) == 2
    assert resp.points[0].ratio == 0.40
    assert resp.average_ratio == 0.50


def test_put_call_ratio_total():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.put_call_ratio(
        start_date="2025-01-01",
        end_date="2025-01-10",
        granularity="total",
    )
    assert isinstance(resp, PutCallRatioResponse)
    assert resp.ticker is None
    assert resp.granularity == "total"
    assert resp.points is None
    assert resp.total_put_volume == 450000
    assert resp.total_call_volume == 900000
    assert resp.total_ratio == 0.50


def test_put_call_ratio_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.put_call_ratio(start_date="", end_date="2025-01-10")

    with pytest.raises(FinTextValidationError):
        client.put_call_ratio(start_date="2025-01-01", end_date="")

    with pytest.raises(FinTextValidationError):
        client.put_call_ratio(start_date="2025-01-01", end_date="2025-01-10", ratio_type="invalid_type")

    with pytest.raises(FinTextValidationError):
        client.put_call_ratio(start_date="2025-01-01", end_date="2025-01-10", granularity="invalid_gran")


def test_earnings_surprise_single_ticker():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.earnings_surprise(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
        min_sentiment_shift=0.15,
        pre_days=5,
        post_days=5,
    )
    assert isinstance(resp, EarningsSurpriseResponse)
    assert resp.ticker == "AAPL"
    assert resp.count == 2
    assert resp.surprises[0].surprise_score == 0.36
    assert resp.surprises[0].direction == "positive"
    assert resp.surprises[1].direction == "negative"


def test_earnings_surprise_universe_scan():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.earnings_surprise(
        min_sentiment_shift=0.15,
        limit=10,
    )
    assert isinstance(resp, EarningsSurpriseResponse)
    assert resp.ticker is None
    assert resp.count == 2
    assert len(resp.surprises) == 2


def test_earnings_surprise_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.earnings_surprise(min_sentiment_shift=0.99)

    with pytest.raises(FinTextValidationError):
        client.earnings_surprise(pre_days=50)

    with pytest.raises(FinTextValidationError):
        client.earnings_surprise(post_days=0)

    with pytest.raises(FinTextValidationError):
        client.earnings_surprise(limit=500)


def test_insider_trading_single_ticker():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.insider_trading(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
        min_shares=5000,
    )
    assert isinstance(resp, InsiderTradingResponse)
    assert resp.ticker == "AAPL"
    assert resp.count > 0
    assert len(resp.trades) > 0
    assert resp.trades[0].ticker == "AAPL"
    assert resp.trades[0].source == "SEC Form 4"


def test_insider_trading_universe_scan():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.insider_trading(
        min_shares=1000,
        limit=10,
    )
    assert isinstance(resp, InsiderTradingResponse)
    assert resp.ticker is None
    assert resp.count == 2
    assert len(resp.trades) == 2


def test_insider_trading_type_filter():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.insider_trading(
        transaction_type="purchase",
    )
    assert isinstance(resp, InsiderTradingResponse)
    assert resp.count == 1
    assert resp.trades[0].transaction_type == "purchase"
    assert resp.trades[0].signal_score > 0.0


def test_insider_trading_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.insider_trading(transaction_type="invalid_type")

    with pytest.raises(FinTextValidationError):
        client.insider_trading(min_shares=-5)

    with pytest.raises(FinTextValidationError):
        client.insider_trading(min_signal_score=1.5)

    with pytest.raises(FinTextValidationError):
        client.insider_trading(limit=500)


def test_sentiment_disagreement_stddev():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.sentiment_disagreement(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
    )
    assert isinstance(resp, SentimentDisagreementResponse)
    assert resp.ticker == "AAPL"
    assert resp.aggregation == "stddev"
    assert resp.disagreement_index > 0.0
    assert resp.record_count == 150
    assert resp.source_count == 3
    assert len(resp.sources_breakdown) == 3


def test_sentiment_disagreement_iqr_and_mad():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp_iqr = client.sentiment_disagreement(
        ticker="NVDA",
        start_date="2025-01-01",
        end_date="2025-03-31",
        aggregation="iqr",
    )
    assert isinstance(resp_iqr, SentimentDisagreementResponse)
    assert resp_iqr.aggregation == "iqr"

    resp_mad = client.sentiment_disagreement(
        ticker="MSFT",
        start_date="2025-01-01",
        end_date="2025-03-31",
        aggregation="mad",
    )
    assert isinstance(resp_mad, SentimentDisagreementResponse)
    assert resp_mad.aggregation == "mad"


def test_sentiment_disagreement_source_filter():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.sentiment_disagreement(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
        source="Finnhub",
    )
    assert isinstance(resp, SentimentDisagreementResponse)
    assert resp.source_count == 1
    assert resp.sources_breakdown[0].source == "Finnhub"


def test_sentiment_disagreement_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.sentiment_disagreement(ticker="", start_date="2025-01-01", end_date="2025-03-31")

    with pytest.raises(FinTextValidationError):
        client.sentiment_disagreement(ticker="AAPL", start_date="", end_date="2025-03-31")

    with pytest.raises(FinTextValidationError):
        client.sentiment_disagreement(ticker="AAPL", start_date="2025-01-01", end_date="")

    with pytest.raises(FinTextValidationError):
        client.sentiment_disagreement(ticker="AAPL", start_date="2025-01-01", end_date="2025-03-31", min_records=2)

    with pytest.raises(FinTextValidationError):
        client.sentiment_disagreement(ticker="AAPL", start_date="2025-01-01", end_date="2025-03-31", aggregation="invalid_agg")


def test_options_vol_surface_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.options_vol_surface(ticker="AAPL")
    assert isinstance(resp, OptionsVolSurfaceResponse)
    assert resp.ticker == "AAPL"
    assert resp.spot == 225.50
    assert len(resp.strikes) == 9
    assert len(resp.expirations) == 3
    assert len(resp.surface) == 3
    for row in resp.surface:
        assert len(row.ivs) == 9


def test_options_vol_surface_custom_params():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.options_vol_surface(
        ticker="NVDA",
        start_date="2025-01-01",
        end_date="2025-06-30",
        strike_range="0.85-1.15",
        strike_count=5,
        risk_free_rate=0.04,
        dividend_yield=0.01,
    )
    assert isinstance(resp, OptionsVolSurfaceResponse)
    assert resp.ticker == "NVDA"
    assert len(resp.strikes) == 5


def test_options_vol_surface_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.options_vol_surface(ticker="")

    with pytest.raises(FinTextValidationError):
        client.options_vol_surface(ticker="AAPL", strike_count=8)

    with pytest.raises(FinTextValidationError):
        client.options_vol_surface(ticker="AAPL", strike_count=17)

    with pytest.raises(FinTextValidationError):
        client.options_vol_surface(ticker="AAPL", risk_free_rate=0.25)

    with pytest.raises(FinTextValidationError):
        client.options_vol_surface(ticker="AAPL", dividend_yield=0.15)


def test_ma_rumors_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.ma_rumors()
    assert isinstance(resp, MARumorsResponse)
    assert resp.ticker is None
    assert resp.count >= 1
    assert resp.items[0].rumor_score >= 0.50
    assert len(resp.items[0].supply_chain_related_tickers) > 0


def test_ma_rumors_custom_filter():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.ma_rumors(ticker="PYPL", min_rumor_score=0.70, lookback_days=14, limit=5)
    assert isinstance(resp, MARumorsResponse)
    assert resp.ticker == "PYPL"
    assert resp.min_rumor_score == 0.70
    assert resp.lookback_days == 14
    assert len(resp.items) == 1
    assert resp.items[0].ticker == "PYPL"
    assert resp.items[0].recent_8k_ma_flag is True


def test_ma_rumors_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.ma_rumors(min_rumor_score=1.5)

    with pytest.raises(FinTextValidationError):
        client.ma_rumors(min_rumor_score=-0.1)

    with pytest.raises(FinTextValidationError):
        client.ma_rumors(lookback_days=45)

    with pytest.raises(FinTextValidationError):
        client.ma_rumors(limit=150)


def test_regulatory_filings_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.regulatory_filings()
    assert isinstance(resp, RegulatoryFilingsResponse)
    assert resp.ticker is None
    assert resp.count >= 1
    assert len(resp.filings) >= 1
    assert resp.filings[0].event_category in ["Annual Report", "M&A", "Quarterly Report", "Other"]


def test_regulatory_filings_custom_filters():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    resp = client.regulatory_filings(
        ticker="AAPL",
        form_type="10-K",
        event_category="Annual Report",
        limit=5,
        offset=0,
    )
    assert isinstance(resp, RegulatoryFilingsResponse)
    assert resp.ticker == "AAPL"
    assert resp.form_type == "10-K"
    assert resp.event_category == "Annual Report"
    assert len(resp.filings) == 1
    assert resp.filings[0].ticker == "AAPL"
    assert resp.filings[0].form_type == "10-K"


def test_regulatory_filings_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.regulatory_filings(limit=150)

    with pytest.raises(FinTextValidationError):
        client.regulatory_filings(offset=-1)


def test_export_parquet_default(tmp_path):
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    data = client.export_parquet(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-01-10",
    )
    assert isinstance(data, bytes)
    assert len(data) >= 8
    assert data.startswith(b"PAR1")
    assert data.endswith(b"PAR1")


def test_export_parquet_with_prices_and_file_save(tmp_path):
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    out_file = tmp_path / "test_export.parquet"
    data = client.export_parquet(
        ticker="NVDA",
        start_date="2025-01-01",
        end_date="2025-01-05",
        include_prices=True,
        min_quality=0.75,
        output_path=out_file,
    )
    assert isinstance(data, bytes)
    assert out_file.exists()
    assert out_file.read_bytes() == data


def test_export_parquet_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.export_parquet(ticker="", start_date="2025-01-01", end_date="2025-01-10")

    with pytest.raises(FinTextValidationError):
        client.export_parquet(ticker="AAPL", start_date="", end_date="2025-01-10")

    with pytest.raises(FinTextValidationError):
        client.export_parquet(ticker="AAPL", start_date="2025-01-01", end_date="")

    with pytest.raises(FinTextValidationError):
        client.export_parquet(ticker="AAPL", start_date="2025-01-01", end_date="2025-01-10", limit=200000)

    with pytest.raises(FinTextValidationError):
        client.export_parquet(ticker="AAPL", start_date="2025-01-01", end_date="2025-01-10", min_quality=1.5)


# ─────────────────────────────────────────────────────────────────────────────
# Billing & Subscription Tests
# ─────────────────────────────────────────────────────────────────────────────

def test_create_checkout_session():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    checkout = client.create_checkout_session("pro_monthly")

    assert isinstance(checkout, CheckoutResponse)
    assert "stripe.com" in checkout.checkout_url
    assert checkout.session_id == "cs_mock_1234567890"


def test_create_checkout_session_with_custom_urls():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    checkout = client.create_checkout_session(
        "enterprise_monthly",
        success_url="https://app.fintext.io/success",
        cancel_url="https://app.fintext.io/cancel",
    )

    assert isinstance(checkout, CheckoutResponse)
    assert "enterprise_monthly" in checkout.checkout_url


def test_create_portal_session():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    portal = client.create_portal_session(return_url="https://app.fintext.io/dashboard")

    assert isinstance(portal, PortalResponse)
    assert "billing.stripe.com" in portal.portal_url


def test_get_subscription():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )
    sub = client.get_subscription()

    assert isinstance(sub, SubscriptionResponse)
    assert sub.plan_id == "free"
    assert sub.plan_name == "Free Tier"
    assert sub.status == "active"
    assert sub.monthly_request_limit == 1000
    assert sub.current_usage == 42


def test_billing_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.create_checkout_session("")

    with pytest.raises(FinTextValidationError):
        client.create_checkout_session("   ")


# ═════════════════════════════════════════════════════════════════════════════
# Tests — Organizations & Teams
# ═════════════════════════════════════════════════════════════════════════════

from fintext import (
    CreateOrgResponse,
    InviteMemberResponse,
    LeaveOrgResponse,
    ListOrgsResponse,
    OrgDetailsResponse,
    RemoveMemberResponse,
    SelectOrgResponse,
    UpdateMemberRoleResponse,
)


def test_create_org():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.create_org("Acme Trading")
    assert isinstance(resp, CreateOrgResponse)
    assert resp.id == "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    assert resp.name == "Acme Trading"
    assert resp.role == "admin"


def test_list_orgs():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.list_orgs()
    assert isinstance(resp, ListOrgsResponse)
    assert resp.count == 1
    assert len(resp.organizations) == 1
    assert resp.organizations[0].name == "Acme Trading"
    assert resp.organizations[0].my_role == "admin"


def test_get_org_details():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.get_org("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
    assert isinstance(resp, OrgDetailsResponse)
    assert resp.id == "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
    assert resp.count == 2
    assert len(resp.members) == 2
    assert resp.members[0].role == "admin"
    assert resp.members[1].user_id == "analyst_42"


def test_invite_member():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.invite_member("org-uuid", "new_user_99", role="viewer")
    assert isinstance(resp, InviteMemberResponse)
    assert resp.status == "ok"
    assert resp.user_id == "new_user_99"
    assert resp.role == "viewer"


def test_update_member_role():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.update_member_role("org-uuid", "analyst_42", "admin")
    assert isinstance(resp, UpdateMemberRoleResponse)
    assert resp.status == "ok"
    assert resp.role == "admin"
    assert resp.user_id == "analyst_42"


def test_remove_member():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.remove_member("org-uuid", "analyst_42")
    assert isinstance(resp, RemoveMemberResponse)
    assert resp.status == "ok"
    assert resp.user_id == "analyst_42"


def test_leave_org():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.leave_org("org-uuid")
    assert isinstance(resp, LeaveOrgResponse)
    assert resp.status == "ok"
    assert resp.org_id == "org-uuid"


def test_select_org_and_token_update():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.select_org("org-uuid")
    assert isinstance(resp, SelectOrgResponse)
    assert resp.status == "ok"
    assert resp.org_id == "org-uuid"
    assert resp.role == "admin"
    assert "jwt_org_scoped_" in resp.token
    # Verify auto-update of client token
    assert client.api_token == resp.token


def test_org_endpoints_require_authentication():
    client = FinTextClient(
        base_url="http://mock.fintext",
        transport=create_mock_transport(),
    )
    with pytest.raises(FinTextAuthError):
        client.create_org("No Auth Org")

    with pytest.raises(FinTextAuthError):
        client.list_orgs()


# ═════════════════════════════════════════════════════════════════════════════
# Tests — Security & IP Whitelisting
# ═════════════════════════════════════════════════════════════════════════════

from fintext import (
    DeleteIpWhitelistResponse,
    IpWhitelistEntry,
    ListIpWhitelistResponse,
)


def test_get_ip_whitelist():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.get_ip_whitelist()
    assert isinstance(resp, ListIpWhitelistResponse)
    assert resp.count == 1
    assert len(resp.entries) == 1
    assert resp.entries[0].ip_or_cidr == "203.0.113.0/24"
    assert resp.entries[0].description == "Primary Office VPN"


def test_add_ip_whitelist():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.add_ip_whitelist("203.0.113.0/24", description="Trading Floor")
    assert isinstance(resp, IpWhitelistEntry)
    assert resp.id == "550e8400-e29b-41d4-a716-446655440000"
    assert resp.ip_or_cidr == "203.0.113.0/24"
    assert resp.description == "Trading Floor"


def test_delete_ip_whitelist():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.delete_ip_whitelist("550e8400-e29b-41d4-a716-446655440000")
    assert isinstance(resp, DeleteIpWhitelistResponse)
    assert resp.status == "success"
    assert resp.id == "550e8400-e29b-41d4-a716-446655440000"


def test_ip_whitelist_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.add_ip_whitelist("")

    with pytest.raises(FinTextValidationError):
        client.add_ip_whitelist("   ")

    with pytest.raises(FinTextValidationError):
        client.delete_ip_whitelist("")

    with pytest.raises(FinTextAPIError):
        client.add_ip_whitelist("invalid_ip_range")


# ═════════════════════════════════════════════════════════════════════════════
# Tests — News Article Full Text Retrieval
# ═════════════════════════════════════════════════════════════════════════════

from fintext import (
    NewsArticleFull,
    NewsArticleMetadata,
    NewsArticlesListResponse,
)


def test_list_news_articles():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.list_news_articles(ticker="AAPL", limit=10, offset=0)
    assert isinstance(resp, NewsArticlesListResponse)
    assert resp.total == 1
    assert len(resp.articles) == 1
    assert resp.articles[0].ticker == "AAPL"
    assert resp.articles[0].sentiment_score == 0.82
    assert len(resp.articles[0].snippet) <= 200


def test_get_news_article():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.get_news_article("550e8400-e29b-41d4-a716-446655440000")
    assert isinstance(resp, NewsArticleFull)
    assert resp.id == "550e8400-e29b-41d4-a716-446655440000"
    assert resp.ticker == "AAPL"
    assert "Apple Inc. announced significant expansion" in resp.full_text
    assert resp.sentiment_score == 0.84


def test_news_articles_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.get_news_article("")

    with pytest.raises(FinTextValidationError):
        client.get_news_article("   ")

    with pytest.raises(FinTextValidationError):
        client.list_news_articles(limit=0)

    with pytest.raises(FinTextValidationError):
        client.list_news_articles(limit=101)

    with pytest.raises(FinTextAPIError):
        client.get_news_article("non_existent_id")


# ═════════════════════════════════════════════════════════════════════════════
# Tests — Entity Sentiment Breakdown
# ═════════════════════════════════════════════════════════════════════════════

from fintext import (
    EntitySentimentItem,
    EntitySentimentResponse,
)


def test_sentiment_entities_default():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_entities()
    assert isinstance(resp, EntitySentimentResponse)
    assert resp.count == 2
    assert len(resp.entities) == 2
    assert resp.entities[0].entity_text == "Nvidia"
    assert resp.entities[0].avg_sentiment == 0.78
    assert resp.entities[0].positive_ratio == 0.90
    assert resp.entities[0].negative_ratio == 0.05
    assert resp.entities[0].mention_count == 14


def test_sentiment_entities_with_filters():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )
    resp = client.sentiment_entities(
        start_date="2026-08-20",
        end_date="2026-08-30",
        entity_type="person",
        min_mentions=5,
        limit=10,
        sort_by="mentions",
    )
    assert isinstance(resp, EntitySentimentResponse)
    assert resp.entity_type == "person"
    assert resp.entities[0].entity_type == "person"


def test_sentiment_entities_validation_errors():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_test_token",
        transport=create_mock_transport(),
    )

    with pytest.raises(FinTextValidationError):
        client.sentiment_entities(min_mentions=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_entities(limit=0)

    with pytest.raises(FinTextValidationError):
        client.sentiment_entities(limit=101)

    with pytest.raises(FinTextValidationError):
        client.sentiment_entities(entity_type="invalid_type")

    with pytest.raises(FinTextValidationError):
        client.sentiment_entities(sort_by="invalid_sort")


def test_get_audit_logs():
    client = FinTextClient(base_url="http://mock.fintext", api_token="test_token", transport=create_mock_transport())
    res = client.get_audit_logs(limit=50)

    assert res.total == 2
    assert len(res.logs) == 2
    assert res.logs[0].action == "apikey.create"
    assert res.logs[0].entity_type == "api_key"
    assert res.logs[1].action == "org.created"

    # Filter by action
    res_filtered = client.get_audit_logs(action="apikey.create")
    assert res_filtered.total == 1
    assert res_filtered.logs[0].action == "apikey.create"


def test_export_audit_logs():
    client = FinTextClient(base_url="http://mock.fintext", api_token="test_token", transport=create_mock_transport())

    # JSON export
    json_res = client.export_audit_logs(format="json")
    assert json_res.total == 1
    assert len(json_res.logs) == 1
    assert json_res.logs[0].user_id == "trader_007"

    # CSV export
    csv_res = client.export_audit_logs(format="csv")
    assert isinstance(csv_res, str)
    assert "id,org_id,user_id,action" in csv_res
    assert "apikey.create" in csv_res

    # Validation errors
    with pytest.raises(FinTextValidationError):
        client.get_audit_logs(limit=0)

    with pytest.raises(FinTextValidationError):
        client.get_audit_logs(limit=1001)

    with pytest.raises(FinTextValidationError):
        client.get_audit_logs(offset=-1)

    with pytest.raises(FinTextValidationError):
        client.export_audit_logs(format="xml")


def test_api_key_lifecycle_and_rotation():
    client = FinTextClient(base_url="http://mock.fintext", api_token="test_token", transport=create_mock_transport())

    # Create API key
    created = client.create_api_key(name="Prod Bot")
    assert isinstance(created, CreateApiKeyResponse)
    assert created.id == "550e8400-e29b-41d4-a716-446655440001"
    assert created.api_key.startswith("ft_")
    assert created.prefix == "ft_12345"

    # List API keys
    keys = client.list_api_keys()
    assert isinstance(keys, ListApiKeysResponse)
    assert keys.count == 1
    assert keys.api_keys[0].rotation_status == "rotating"
    assert keys.api_keys[0].expires_at is not None

    # Get single API key details
    key_details = client.get_api_key("550e8400-e29b-41d4-a716-446655440001")
    assert isinstance(key_details, ApiKeyItem)
    assert key_details.id == "550e8400-e29b-41d4-a716-446655440001"

    # Rotate API key
    rotated = client.rotate_api_key("550e8400-e29b-41d4-a716-446655440001", overlap_hours=48)
    assert isinstance(rotated, RotateApiKeyResponse)
    assert rotated.id == "550e8400-e29b-41d4-a716-446655440002"
    assert rotated.rotated_from == "550e8400-e29b-41d4-a716-446655440001"
    assert rotated.overlap_hours == 48

    # Revoke API key
    revoked = client.revoke_api_key("550e8400-e29b-41d4-a716-446655440001")
    assert revoked["status"] == "revoked"

    # Validation errors
    with pytest.raises(FinTextValidationError):
        client.get_api_key("")

    with pytest.raises(FinTextValidationError):
        client.rotate_api_key("", overlap_hours=24)

    with pytest.raises(FinTextValidationError):
        client.rotate_api_key("550e8400-e29b-41d4-a716-446655440001", overlap_hours=0)

    with pytest.raises(FinTextValidationError):
        client.rotate_api_key("550e8400-e29b-41d4-a716-446655440001", overlap_hours=200)

    with pytest.raises(FinTextValidationError):
        client.revoke_api_key("")


def test_unified_search():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_jwt_token",
        transport=create_mock_transport(),
    )

    # 1. Search for AAPL
    resp = client.search(q="AAPL")
    assert isinstance(resp, SearchResponse)
    assert resp.query == "AAPL"
    assert resp.count == 2
    assert len(resp.results) == 2
    assert resp.results[0].ticker == "AAPL"
    assert resp.results[0].score == 1.0

    # 2. Filter types
    resp_filtered = client.search(q="AAPL", types=["news", "transcripts"], limit=10, offset=0)
    assert resp_filtered.types == ["news", "transcripts"]

    # 3. Validation errors
    with pytest.raises(FinTextValidationError):
        client.search(q="")

    with pytest.raises(FinTextValidationError):
        client.search(q="AAPL", limit=0)

    with pytest.raises(FinTextValidationError):
        client.search(q="AAPL", limit=101)

    with pytest.raises(FinTextValidationError):
        client.search(q="AAPL", offset=-1)


def test_model_metadata_and_provenance_models():
    """Verify ModelMetadata model and optional metadata lineage fields on SentimentResponse."""
    from fintext.models import (
        ModelMetadata,
        SentimentResponse,
        SentimentFeedResponse,
        SentimentFeedItem,
        SentimentHistoryResponse,
        SentimentRecord,
    )

    # 1. Test ModelMetadata defaults
    meta = ModelMetadata()
    assert meta.model_version == "finbert-v3.1.0"
    assert meta.pipeline_version == "2.0.0"
    assert meta.data_provenance == ["SEC EDGAR", "Finnhub", "Polygon"]

    # 2. Test deserialization of payload without metadata (backward compatibility)
    legacy_payload = {
        "ticker": "AAPL",
        "date": "2026-08-30",
        "sentiment_score": 0.85,
        "sentiment_label": "BULLISH",
        "confidence": 0.92,
        "signal_available_ts_us": 1787940389786186,
        "data_quality_score": 0.90,
        "message": "Signal retrieved",
    }
    resp_legacy = SentimentResponse.model_validate(legacy_payload)
    assert resp_legacy.ticker == "AAPL"
    assert resp_legacy.model_version is None
    assert resp_legacy.pipeline_version is None
    assert resp_legacy.data_provenance is None

    # 3. Test deserialization of payload with metadata
    modern_payload = {
        **legacy_payload,
        "model_version": "finbert-v3.1.0",
        "pipeline_version": "2.0.0",
        "data_provenance": ["SEC EDGAR", "Finnhub", "Polygon"],
    }
    resp_modern = SentimentResponse.model_validate(modern_payload)
    assert resp_modern.model_version == "finbert-v3.1.0"
    assert resp_modern.pipeline_version == "2.0.0"
    assert resp_modern.data_provenance == ["SEC EDGAR", "Finnhub", "Polygon"]

    # 4. Test SentimentFeedItem & SentimentFeedResponse metadata
    item_payload = {
        "published_utc": "2026-08-30T10:00:00Z",
        "ticker": "AAPL",
        "source": "Bloomberg",
        "title": "AAPL analysis",
        "sentiment_score": 0.70,
        "sentiment_label": "BULLISH",
        "confidence": 0.85,
        "data_quality_score": 0.92,
        "vpin": 0.25,
        "gamma_exposure": 150000.0,
        "model_version": "finbert-v3.1.0",
        "pipeline_version": "2.0.0",
        "data_provenance": ["Bloomberg"],
    }
    item = SentimentFeedItem.model_validate(item_payload)
    assert item.model_version == "finbert-v3.1.0"
    assert item.data_provenance == ["Bloomberg"]


def test_kafka_streaming_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. List topics
    topics_resp = client.list_kafka_topics()
    assert isinstance(topics_resp, KafkaTopicsResponse)
    assert topics_resp.total_topics == 3
    assert len(topics_resp.topics) == 3
    assert topics_resp.topics[0].topic == "sentiment-events"

    # 2. Get credentials
    creds = client.get_kafka_credentials(
        topic="sentiment-events",
        ttl_minutes=45,
        consumer_group="custom_sync_cg",
    )
    assert isinstance(creds, KafkaCredentials)
    assert creds.topic == "sentiment-events"
    assert creds.consumer_group == "custom_sync_cg"
    assert creds.password.startswith("sec_")

    # 3. Revoke credentials
    rev = client.revoke_kafka_credentials(creds.id)
    assert isinstance(rev, RevokeKafkaCredentialsResponse)
    assert rev.status == "revoked"
    assert rev.id == creds.id


def test_retention_policy_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Get policies
    policies_resp = client.get_retention_policies()
    assert isinstance(policies_resp, RetentionPoliciesResponse)
    assert policies_resp.total_policies == 1
    assert len(policies_resp.policies) == 1
    assert policies_resp.policies[0].data_category == "usage_events"

    # 2. Create policy
    new_policy = client.create_retention_policy(
        data_category="audit_logs",
        retention_days=180,
        is_active=True,
    )
    assert isinstance(new_policy, RetentionPolicy)
    assert new_policy.data_category == "audit_logs"
    assert new_policy.retention_days == 180

    # 3. Delete policy
    del_resp = client.delete_retention_policy(new_policy.id)
    assert isinstance(del_resp, DeleteRetentionPolicyResponse)
    assert del_resp.status == "deleted"
    assert del_resp.id == new_policy.id


def test_factor_exposure_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.factor_exposure(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-06-30",
        factors=["market", "momentum", "sentiment", "volatility"],
        benchmark_ticker="SPY",
    )

    assert isinstance(resp, FactorExposureResponse)
    assert resp.ticker == "AAPL"
    assert resp.benchmark_ticker == "SPY"
    assert len(resp.factors_included) == 4
    assert len(resp.exposures) == 4
    assert resp.ols_summary.num_observations == 120
    assert resp.ols_summary.r_squared > 0.0


def test_esg_scores_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.esg_scores(
        ticker="AAPL",
        start_date="2025-06-01",
        end_date="2025-08-30",
        min_confidence=0.5,
    )

    assert isinstance(resp, ESGScoresResponse)
    assert resp.ticker == "AAPL"
    assert resp.overall_esg_score == 68.5
    assert resp.dimensions.environmental.mention_count == 12
    assert resp.dimensions.social.score == 0.35
    assert resp.dimensions.governance.positive_ratio == 0.70


def test_bankruptcy_risk_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.bankruptcy_risk(
        ticker="AAPL",
        lookback_days=30,
        include_components=True,
    )

    assert isinstance(resp, BankruptcyRiskResponse)
    assert resp.ticker == "AAPL"
    assert resp.lookback_days == 30
    assert resp.bankruptcy_risk_score == 32.5
    assert resp.risk_category == "MODERATE"
    assert resp.components is not None
    assert resp.components.eight_k_distress_score == 10.0
    assert resp.components.sentiment_deterioration_score == 5.0


def test_fx_sentiment_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.fx_sentiment(
        currency_pair="EUR/USD",
        start_date="2025-08-01",
        end_date="2025-08-31",
        min_confidence=0.5,
        limit=10,
    )

    assert isinstance(resp, FXSentimentResponse)
    assert resp.currency_pair == "EUR/USD"
    assert resp.summary.mention_count == 18
    assert resp.summary.avg_sentiment == 0.28
    assert resp.summary.positive_ratio == 0.65
    assert len(resp.top_articles) == 1
    assert resp.top_articles[0].source == "Reuters FX"


def test_commodity_sentiment_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.commodity_sentiment(
        commodity="crude_oil",
        start_date="2025-08-01",
        end_date="2025-08-31",
        min_confidence=0.5,
        limit=10,
    )

    assert isinstance(resp, CommoditySentimentResponse)
    assert resp.commodity == "crude_oil"
    assert resp.summary.mention_count == 25
    assert resp.summary.avg_sentiment == 0.35
    assert resp.summary.positive_ratio == 0.60
    assert len(resp.top_articles) == 1
    assert resp.top_articles[0].source == "Reuters Commodities"


def test_polling_webhooks_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    created = client.create_polling_webhook(
        name="Test Sentiment Poll",
        url="https://quant.fund.com/poll",
        interval_seconds=300,
        query_type="sentiment",
        query_params={"tickers": ["AAPL", "MSFT"]},
    )
    assert isinstance(created, PollingWebhook)
    assert created.name == "Test Sentiment Poll"
    assert len(created.secret) == 64

    listing = client.list_polling_webhooks()
    assert isinstance(listing, PollingWebhooksResponse)
    assert listing.total >= 1
    assert len(listing.webhooks) >= 1

    deleted = client.delete_polling_webhook(created.id)
    assert isinstance(deleted, DeletePollingWebhookResponse)
    assert deleted.success is True
    assert deleted.id == created.id


def test_crypto_sentiment_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.crypto_sentiment(
        asset="BTC",
        start_date="2025-08-01",
        end_date="2025-08-31",
        min_confidence=0.5,
        limit=10,
    )

    assert isinstance(resp, CryptoSentimentResponse)
    assert resp.asset == "BTC"
    assert resp.summary.mention_count == 32
    assert resp.summary.avg_sentiment == 0.58
    assert resp.summary.positive_ratio == 0.72
    assert len(resp.top_articles) == 1
    assert resp.top_articles[0].source == "CoinDesk"


def test_options_microstructure_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.options_microstructure(
        ticker="AAPL",
        start_date="2025-08-01",
        end_date="2025-08-31",
        metric="both",
        interval="daily",
        limit=100,
    )

    assert isinstance(resp, MicrostructureResponse)
    assert resp.ticker == "AAPL"
    assert resp.metric == "both"
    assert resp.interval == "daily"
    assert resp.count == 2
    assert len(resp.points) == 2
    assert resp.points[0].vpin == 0.235
    assert resp.points[0].gex == 1250000.0


def test_market_breadth_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.market_breadth(
        start_date="2025-01-01",
        end_date="2025-01-15",
        universe="all",
        limit=50,
        include_new_highs_lows=True,
    )

    assert isinstance(resp, MarketBreadthResponse)
    assert resp.start_date == "2025-01-01"
    assert resp.end_date == "2025-01-15"
    assert resp.universe == "all"
    assert resp.total_tickers == 500
    assert resp.count == 2
    assert len(resp.points) == 2
    assert resp.points[0].advancers == 300
    assert resp.points[0].decliners == 180
    assert resp.points[0].advance_decline_ratio == 0.625
    assert resp.points[0].breadth_index == 0.24
    assert resp.points[0].new_52w_highs == 25
    assert resp.points[0].new_52w_lows == 10


def test_chat_alerts_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Create Telegram chat alert
    sub = client.create_chat_alert(
        channel_type="telegram",
        channel_target="123456789",
        event_types=["sentiment_anomaly", "8k_filing"],
    )
    assert isinstance(sub, ChatAlertSubscription)
    assert sub.id == "550e8400-e29b-41d4-a716-446655440000"
    assert sub.channel_type == "telegram"
    assert sub.channel_target == "123456789"
    assert "sentiment_anomaly" in sub.event_types

    # 2. List chat alerts
    listed = client.list_chat_alerts()
    assert isinstance(listed, ChatAlertsResponse)
    assert listed.total >= 1
    assert len(listed.subscriptions) >= 1
    assert listed.subscriptions[0].channel_type == "telegram"

    # 3. Delete chat alert
    del_res = client.delete_chat_alert(sub.id)
    assert isinstance(del_res, DeleteChatAlertResponse)
    assert del_res.success is True
    assert del_res.id == sub.id


def test_credit_sentiment_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.credit_sentiment(ticker="AAPL", lookback_days=30)
    assert isinstance(resp, CreditSentimentResponse)
    assert resp.ticker == "AAPL"
    assert resp.lookback_days == 30
    assert resp.credit_sentiment_score == 0.25
    assert resp.news_sentiment_avg == 0.35
    assert resp.eight_k_distress_count == 0
    assert resp.put_call_ratio == 0.75
    assert resp.implied_volatility == 0.28


def test_backfill_sentiment_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.backfill_sentiment(
        ticker="AAPL",
        start_date="2025-01-01",
        end_date="2025-03-31",
        limit=500,
        overwrite=False,
    )
    assert isinstance(resp, BackfillSentimentResponse)
    assert resp.ticker == "AAPL"
    assert resp.start_date == "2025-01-01"
    assert resp.end_date == "2025-03-31"
    assert resp.overwrite is False
    assert resp.total_articles_found == 150
    assert resp.processed_articles == 150
    assert resp.failed_articles == 0


def test_portfolio_optimize_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.portfolio_optimize(
        tickers=["AAPL", "MSFT", "NVDA"],
        start_date="2025-01-01",
        end_date="2025-06-30",
        optimization_type="max_sharpe",
        risk_free_rate=0.05,
        long_only=True,
    )
    assert isinstance(resp, PortfolioOptimizeResponse)
    assert len(resp.tickers) == 3
    assert resp.optimization_type == "max_sharpe"
    assert resp.risk_free_rate == 0.05
    assert len(resp.weights) == 3
    assert resp.expected_annual_return == 0.15
    assert resp.expected_annual_volatility == 0.18
    assert resp.sharpe_ratio == 0.55


def test_portfolio_factor_exposure_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    resp = client.portfolio_factor_exposure(
        tickers=["AAPL", "MSFT", "NVDA"],
        weights=[0.4, 0.3, 0.3],
        start_date="2025-01-01",
        end_date="2025-06-30",
        benchmark_ticker="SPY",
        factors=["market", "momentum", "sentiment", "volatility"],
    )
    assert isinstance(resp, PortfolioFactorExposureResponse)
    assert resp.tickers == ["AAPL", "MSFT", "NVDA"]
    assert resp.weights == [0.4, 0.3, 0.3]
    assert resp.benchmark_ticker == "SPY"
    assert len(resp.exposures) == 4
    assert resp.ols_summary.r_squared == 0.75
    assert resp.exposures[0].factor == "market"
    assert resp.exposures[0].beta == 1.05


def test_retraining_jobs_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Create retraining job
    create_resp = client.create_retraining_job(
        model_type="sentiment",
        trigger_type="manual",
        config={"learning_rate": 2e-5, "epochs": 3, "batch_size": 32},
    )
    assert isinstance(create_resp, RetrainingJobResponse)
    assert create_resp.job.model_type == "sentiment"
    assert create_resp.job.status == "pending"
    assert create_resp.job.config["epochs"] == 3
    assert create_resp.message == "Retraining job queued successfully"

    job_id = create_resp.job.id

    # 2. List retraining jobs
    list_resp = client.list_retraining_jobs(status="completed", limit=10, offset=0)
    assert isinstance(list_resp, ListRetrainingJobsResponse)
    assert len(list_resp.jobs) >= 1
    assert list_resp.total >= 1
    assert list_resp.limit == 50

    # 3. Get retraining job
    get_resp = client.get_retraining_job(job_id=job_id)
    assert isinstance(get_resp, RetrainingJobResponse)
    assert get_resp.job.id == job_id

    # 4. Cancel retraining job
    cancel_resp = client.cancel_retraining_job(job_id=job_id)
    assert isinstance(cancel_resp, RetrainingJobResponse)
    assert cancel_resp.job.status == "cancelled"
    assert cancel_resp.message == "Retraining job cancelled successfully"


def test_fix_orders_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Submit Market Order (Buy AAPL)
    raw_fix_market = "8=FIX.4.4|9=120|35=D|11=ORD-12345|55=AAPL|54=1|38=100|40=1|10=000|"
    order_resp = client.submit_fix_order(raw_fix_market)
    assert isinstance(order_resp, FIXOrderResponse)
    assert order_resp.cl_ord_id == "ORD-12345"
    assert order_resp.symbol == "AAPL"
    assert order_resp.side == "1"
    assert order_resp.order_type == "1"
    assert order_resp.qty == 100.0
    assert order_resp.filled_qty == 100.0
    assert order_resp.status == "filled"
    assert order_resp.exec_type == "2"
    assert "35=8" in order_resp.fix_message

    # 2. Submit order using FIXOrderRequest wrapper
    req = FIXOrderRequest(fix_message=raw_fix_market)
    order_resp2 = client.submit_fix_order(req)
    assert order_resp2.cl_ord_id == "ORD-12345"

    # 3. List FIX Orders
    orders_list = client.list_fix_orders(status="filled", limit=10, offset=0)
    assert isinstance(orders_list, FIXOrdersListResponse)
    assert len(orders_list.orders) >= 1
    assert orders_list.orders[0].symbol == "AAPL"
    assert orders_list.orders[0].status == "filled"

    # 4. Cancel FIX Order
    cancel_fix = "8=FIX.4.4|9=110|35=F|11=ORD-CANCEL-01|41=ORD-12346|55=MSFT|54=2|38=50|10=000|"
    cancel_resp = client.cancel_fix_order(cancel_fix)
    assert isinstance(cancel_resp, FIXOrderResponse)
    assert cancel_resp.status == "cancelled"
    assert cancel_resp.exec_type == "4"
    assert cancel_resp.symbol == "MSFT"

    # 5. Cancel order using FIXCancelRequest wrapper
    cancel_req = FIXCancelRequest(fix_message=cancel_fix)
    cancel_resp2 = client.cancel_fix_order(cancel_req)
    assert cancel_resp2.status == "cancelled"


def test_dlq_monitoring_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. List DLQ events
    events_list = client.list_dlq_events(source="sentiment", status="failed", limit=10, offset=0)
    assert isinstance(events_list, DLQEventsListResponse)
    assert len(events_list.events) >= 1
    assert events_list.events[0].source == "sentiment"
    assert events_list.events[0].retry_count == 3

    # 2. Get DLQ event details
    target_id = "550e8400-e29b-41d4-a716-446655440101"
    detail = client.get_dlq_event(target_id)
    assert isinstance(detail, DLQEventDetail)
    assert detail.id == target_id
    assert detail.source == "sentiment"
    assert "ticker" in detail.payload

    # 3. Reprocess DLQ event
    reproc = client.reprocess_dlq_event(target_id)
    assert isinstance(reproc, ReprocessDLQResponse)
    assert reproc.id == target_id
    assert reproc.status == "reprocessed"
    assert reproc.retry_count == 4

    # 4. Purge DLQ event
    purge = client.purge_dlq_event(target_id)
    assert isinstance(purge, PurgeDLQResponse)
    assert purge.id == target_id
    assert purge.status == "purged"


def test_sla_status_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Default SLA status query
    resp = client.sla_status()
    assert isinstance(resp, SLAStatusResponse)
    assert resp.total_requests == 3000
    assert resp.average_latency_ms == 5.25
    assert "p50" in resp.percentiles
    assert "p95" in resp.percentiles
    assert "p99" in resp.percentiles
    assert resp.sla_target_ms == 100
    assert resp.sla_compliance_rate == 99.9
    assert resp.sla_status == "met"

    # 2. Custom percentiles and SLA target
    resp_custom = client.sla_status(
        start_date="2025-08-01",
        end_date="2025-08-31",
        percentiles="50,75,90,99.5",
        sla_target_ms=10,
    )
    assert isinstance(resp_custom, SLAStatusResponse)
    assert "p50" in resp_custom.percentiles
    assert "p75" in resp_custom.percentiles
    assert "p90" in resp_custom.percentiles
    assert "p99.5" in resp_custom.percentiles
    assert resp_custom.sla_target_ms == 10
    assert resp_custom.sla_status == "breached"


def test_sandbox_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Get Sandbox Status
    status = client.get_sandbox_status()
    assert isinstance(status, SandboxStatusResponse)
    assert status.active is True
    assert status.mock_data_version == "sandbox-v1.0"
    assert len(status.available_endpoints) >= 1

    # 2. Activate Sandbox
    act = client.activate_sandbox()
    assert isinstance(act, SandboxStatusResponse)
    assert act.active is True
    assert act.activated_at is not None

    # 3. Deactivate Sandbox
    deact = client.deactivate_sandbox()
    assert isinstance(deact, SandboxStatusResponse)
    assert deact.active is False
    assert deact.deactivated_at is not None


def test_provenance_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Sentiment Provenance
    resp_sent = client.get_provenance("sentiment", "AAPL_2026-08-30T10:15:00Z")
    assert isinstance(resp_sent, DataProvenanceResponse)
    assert resp_sent.record_type == "sentiment"
    assert resp_sent.record_id == "AAPL_2026-08-30T10:15:00Z"
    assert len(resp_sent.provenance_entries) == 1
    assert resp_sent.provenance_entries[0].source_type == "finnhub"
    assert resp_sent.provenance_entries[0].model_version == "finbert-v3.1.0"
    assert len(resp_sent.provenance_entries[0].processing_steps) == 5
    assert resp_sent.provenance_entries[0].processing_steps[0].step == "fetch_article"
    assert resp_sent.provenance_entries[0].processing_steps[3].step == "infer_sentiment"

    # 2. News Article Provenance
    resp_news = client.get_provenance("news", "550e8400-e29b-41d4-a716-446655440000")
    assert isinstance(resp_news, DataProvenanceResponse)
    assert resp_news.record_type == "news"
    assert resp_news.record_id == "550e8400-e29b-41d4-a716-446655440000"
    assert resp_news.provenance_entries[0].source_type == "sec_edgar"
    assert resp_news.provenance_entries[0].processing_steps[0].step == "fetch_source"
    assert resp_news.provenance_entries[0].processing_steps[4].step == "publish_index"


def test_anomaly_scan_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Trigger Anomaly Scan
    resp = client.trigger_anomaly_scan()
    assert isinstance(resp, AnomalyScanResponse)
    assert resp.anomalies_found == 2
    assert resp.alerts_broadcasted == 2
    assert len(resp.anomalies) == 2
    assert resp.anomalies[0].type == "sentiment_anomaly"
    assert resp.anomalies[0].ticker == "AAPL"
    assert resp.anomalies[0].zscore == 3.48
    assert resp.anomalies[0].direction == "bullish"
    assert resp.anomalies[1].ticker == "NVDA"
    assert resp.anomalies[1].direction == "bearish"

    # 2. Test WebSocket URL generator
    ws_url = client.get_anomaly_websocket_url()
    assert ws_url.startswith("ws://mock.fintext/ws?")
    assert "streams=anomalies" in ws_url
    assert "token=valid_bearer_token" in ws_url


def test_provider_health_sync_client():
    client = FinTextClient(
        base_url="http://mock.fintext",
        api_token="valid_bearer_token",
        transport=create_mock_transport(),
    )

    # 1. Fetch all providers
    resp = client.get_provider_health()
    assert isinstance(resp, ProviderHealthResponse)
    assert len(resp.providers) == 3
    assert resp.generated_at == "2026-09-05T11:00:00Z"

    sec = next(p for p in resp.providers if p.provider == "sec_edgar")
    assert sec.status == "healthy"
    assert sec.requests_total == 120
    assert sec.requests_success == 118
    assert sec.success_rate_pct == 98.33
    assert sec.avg_latency_ms == 250.5
    assert sec.p95_latency_ms == 480.0
    assert sec.error_count_last_hour == 2
    assert sec.last_error_message == "Timeout after 5000ms"

    # 2. Filter by single provider
    resp_finnhub = client.provider_health(provider="finnhub", window_minutes=30)
    assert len(resp_finnhub.providers) == 1
    assert resp_finnhub.providers[0].provider == "finnhub"
    assert resp_finnhub.providers[0].status == "healthy"

    # 3. Invalid provider
    with pytest.raises(FinTextAPIError) as exc_info:
        client.get_provider_health(provider="unknown_provider")
    assert exc_info.value.status_code == 400

    # 4. Invalid window_minutes
    with pytest.raises(FinTextAPIError) as exc_info:
        client.get_provider_health(window_minutes=0)
    assert exc_info.value.status_code == 400






