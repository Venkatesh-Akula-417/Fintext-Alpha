"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Asynchronous Client
═══════════════════════════════════════════════════════════════════════════════
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any, BinaryIO, Optional, Union
from urllib.parse import urlencode, urlparse, urlunparse

import httpx

from fintext.exceptions import (
    FinTextAPIError,
    FinTextAuthError,
    FinTextConnectionError,
    FinTextRateLimitError,
    FinTextValidationError,
)
from fintext.models import (
    AcousticFeatures,
    ApiKeyItem,
    AudioSentiment,
    AudioTranscriptionResponse,
    AuditLogExportResponse,
    AuditLogsResponse,
    BacktestRequest,
    BacktestResponse,
    BatchSentimentResponse,
    CheckoutResponse,
    CreateApiKeyResponse,
    DeleteIpWhitelistResponse,
    DeleteTranscriptResponse,
    DeleteUniverseResponse,
    EarningsSurpriseItem,
    EarningsSurpriseResponse,
    EightKFiling,
    EightKResponse,
    EntitySentimentItem,
    EntitySentimentResponse,
    EventStudyResponse,
    HealthResponse,
    InsiderTradeItem,
    InsiderTradingResponse,
    IpWhitelistEntry,
    IssueTokenRequest,
    IssueTokenResponse,
    ListApiKeysResponse,
    ListIpWhitelistResponse,
    ListUniversesResponse,
    MARumorItem,
    MARumorsResponse,
    MarketRegimeResponse,
    NewsArticleFull,
    NewsArticleMetadata,
    NewsArticlesListResponse,
    OptionContract,
    OptionsIvResponse,
    OptionsVolSurfaceResponse,
    PortalResponse,
    PutCallRatioResponse,
    RateLimitInfo,
    RegimeComponents,
    RegulatoryFilingItem,
    RegulatoryFilingsResponse,
    ReturnCorrelationItem,
    ReturnCorrelationResponse,
    RevisionHistoryListResponse,
    RevisionIngestRequest,
    RevisionIngestResponse,
    RotateApiKeyResponse,
    SearchResponse,
    SearchResultItem,
    SectorSentimentResponse,
    SecurityIdentifiers,
    SentimentAnomaliesResponse,
    SentimentAnomalyItem,
    SentimentDisagreementResponse,
    SentimentFeedItem,
    SentimentFeedResponse,
    SentimentHistoryResponse,
    SentimentResponse,
    SpilloverMatrixResponse,
    SpilloverResponse,
    SupplyChainRiskResponse,
    CreateOrgResponse,
    InviteMemberResponse,
    LeaveOrgResponse,
    ListOrgsResponse,
    OrgDetailsResponse,
    RemoveMemberResponse,
    SelectOrgResponse,
    SubscriptionResponse,
    SymbolMapResponse,
    TranscriptListResponse,
    TranscriptMetadata,
    TranscriptResponse,
    Universe,
    UnusualOptionsResponse,
    UpdateMemberRoleResponse,
    UsageStatsResponse,
    SectorRotationItem,
    SectorRotationResponse,
    DigestSubscription,
    CreateDigestRequest,
    DigestSubscriptionResponse,
    DeleteDigestResponse,
    TriggerDigestRequest,
    DigestItemCounts,
    TriggerDigestResponse,
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
    SLALatencyResponse,
    StageBreakdown,
    SandboxStatusResponse,
    SandboxStatus,
    DataProvenanceResponse,
    DataProvenance,
    ProcessingStep,
    DataProvenanceItem,
    AnomalyScanResponse,
    SentimentAnomalyAlert,
    LanguageDetectionResponse,
    ModelCardResponse,
    ModelCard,
    AlphaSignalConfig,
    AlphaReportResponse,
    AlphaReport,
    PITReplayNewsItem,
    PITReplayFilingItem,
    PITReplayEventItem,
    PITReplaySentimentItem,
    PITReplaySummary,
    PITReplayConsistency,
    PITReplayResponse,
    PITReplay,
    SignalQualityReportRequest,
    SignalQualityReportResponse,
    SignalQualityReport,
    ICSummary,
    DecayCurvePoint,
    MarketCapBias,
    PITCertificateParams,
    PITCertificateResponse,
    PITCertificate,
    PITCertificateTests,
    PITCertificatePolicies,
    PITTestResult,
    PITDuplicateTestResult,
    PITBackfillTestResult,
    ProviderHealthItem,
    ProviderHealthResponse,
    ProviderHealth,
    CalibrationPoint,
    ClassConfusion,
    ClassificationMetrics,
    ConfusionMatrix,
    ModelValidationResponse,
    ModelValidation,
    PerClassMetrics,
)


def decode_websocket_frame(data: Union[str, bytes]) -> Any:
    """
    Decode a WebSocket streaming frame received from FinText Alpha Vectorizer.

    Automatically decodes binary MessagePack frames (when connected with format='msgpack')
    or JSON text strings (when connected with format='json').

    Args:
        data: Raw WebSocket frame payload (str for text frames, bytes for binary frames).

    Returns:
        Deserialized dictionary or list representing the message payload.
    """
    if isinstance(data, bytes):
        import msgpack
        return msgpack.unpackb(data, raw=False)
    elif isinstance(data, str):
        import json
        return json.loads(data)
    else:
        raise TypeError(f"Expected str or bytes for WebSocket frame, got {type(data).__name__}")


class FinTextAsyncClient:
    """
    Asynchronous Python Client for the FinText Alpha Vectorizer API.

    Handles async authentication, token lifecycle, rate limit tracking,
    and high-concurrency event retrieval using httpx.AsyncClient.

    Args:
        base_url: Base URL of the FinText API server (defaults to FINTEXT_BASE_URL or 'http://127.0.0.1:8000').
        api_token: Bearer JWT token for authenticated endpoints (defaults to FINTEXT_API_TOKEN).
        admin_token: Admin token for issuing JWTs via POST /auth/token (defaults to FINTEXT_ADMIN_TOKEN).
        timeout: HTTP request timeout in seconds (default: 10.0s).
        auto_auth_user: Default user_id used when auto-requesting a JWT using admin_token.
        transport: Optional custom Async HTTP transport (useful for mocking and unit testing).
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        api_token: Optional[str] = None,
        admin_token: Optional[str] = None,
        api_version: Optional[str] = None,
        timeout: float = 10.0,
        auto_auth_user: str = "python_sdk_client",
        transport: Optional[httpx.AsyncBaseTransport] = None,
    ) -> None:
        raw_url = (
            base_url
            or os.getenv("FINTEXT_BASE_URL")
            or "http://127.0.0.1:8000"
        ).rstrip("/")
        version = api_version or os.getenv("FINTEXT_API_VERSION")
        if version:
            v_prefix = f"/{version.strip('/')}"
            if not raw_url.endswith(v_prefix):
                raw_url = f"{raw_url}{v_prefix}"
        self.base_url = raw_url
        self.api_version = version
        self.api_token = api_token or os.getenv("FINTEXT_API_TOKEN")
        self.admin_token = admin_token or os.getenv("FINTEXT_ADMIN_TOKEN")
        self.timeout = timeout
        self.auto_auth_user = auto_auth_user

        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            timeout=self.timeout,
            transport=transport,
        )
        self._last_rate_limit = RateLimitInfo()

    @property
    def last_rate_limit(self) -> RateLimitInfo:
        """Returns the rate limit metadata extracted from the most recent API response."""
        return self._last_rate_limit

    def set_token(self, token: str) -> None:
        """Manually update or set the active Bearer JWT token."""
        self.api_token = token.strip() if token else None

    def set_admin_token(self, token: str) -> None:
        """Set or update the admin token for token issuance."""
        self.admin_token = token.strip() if token else None

    def _update_rate_limit_headers(self, headers: httpx.Headers) -> None:
        def _get_int(key: str) -> Optional[int]:
            val = headers.get(key)
            if val is not None and val.isdigit():
                return int(val)
            return None

        self._last_rate_limit = RateLimitInfo(
            limit=_get_int("x-ratelimit-limit"),
            remaining=_get_int("x-ratelimit-remaining"),
            reset=_get_int("x-ratelimit-reset"),
        )

    def _handle_response_error(self, response: httpx.Response) -> None:
        self._update_rate_limit_headers(response.headers)
        status = response.status_code

        try:
            body = response.json()
            error_cat = body.get("error", response.reason_phrase or "Error")
            msg = body.get("message", body.get("error", response.text))
        except Exception:
            body = None
            error_cat = response.reason_phrase or "Error"
            msg = response.text

        headers_dict = dict(response.headers)

        if status == 429:
            retry_after = None
            raw_retry = response.headers.get("retry-after")
            if raw_retry and raw_retry.isdigit():
                retry_after = int(raw_retry)

            raise FinTextRateLimitError(
                status_code=429,
                error=error_cat,
                message=msg,
                retry_after=retry_after,
                limit=self._last_rate_limit.limit,
                remaining=self._last_rate_limit.remaining,
                reset=self._last_rate_limit.reset,
                response_body=body,
                headers=headers_dict,
            )

        if status in (401, 403):
            raise FinTextAuthError(
                status_code=status,
                error=error_cat,
                message=msg,
                response_body=body,
                headers=headers_dict,
            )

        raise FinTextAPIError(
            status_code=status,
            error=error_cat,
            message=msg,
            response_body=body,
            headers=headers_dict,
        )

    async def _ensure_authenticated(self) -> None:
        """Automatically acquires a JWT if no api_token is set but admin_token is available."""
        if not self.api_token and self.admin_token:
            await self.get_token(user_id=self.auto_auth_user)
        elif not self.api_token:
            raise FinTextAuthError(
                status_code=401,
                error="Unauthorized",
                message="Authentication required: Provide an 'api_token' or 'admin_token' to access protected endpoints.",
            )

    async def health(self) -> HealthResponse:
        """
        Check the API server health and operational status asynchronously.

        Returns:
            HealthResponse with status, version, and server timestamp.
        """
        try:
            resp = await self._client.get("/health")
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return HealthResponse.model_validate(resp.json())

    async def get_token(
        self,
        user_id: str = "python_sdk_client",
        expires_in_seconds: int = 3600,
        role: str = "institutional",
    ) -> str:
        """
        Request a temporary JWT token from POST /auth/token asynchronously.

        Requires that `admin_token` is configured.

        Args:
            user_id: Target user or institutional account identifier.
            expires_in_seconds: Token lifetime in seconds (default: 3600).
            role: Client role (default: "institutional").

        Returns:
            The issued JWT token string.
        """
        if not user_id.strip():
            raise FinTextValidationError("Field 'user_id' cannot be empty.")

        headers = {}
        if self.admin_token:
            headers["X-Admin-Token"] = self.admin_token

        payload = IssueTokenRequest(
            user_id=user_id,
            expires_in_seconds=expires_in_seconds,
            role=role,
        ).model_dump(exclude_none=True)

        try:
            resp = await self._client.post("/auth/token", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        data = IssueTokenResponse.model_validate(resp.json())
        self.api_token = data.token
        return data.token

    async def sentiment(
        self,
        ticker: str,
        date: Optional[str] = None,
        as_of_utc: Optional[str] = None,
    ) -> SentimentResponse:
        """
        Query point-in-time financial sentiment for a stock ticker asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            date: Optional historical date string formatted as YYYY-MM-DD (defaults to latest).
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            SentimentResponse with score, classification label, and signal timestamp.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")

        await self._ensure_authenticated()

        params = {"ticker": ticker.strip().upper()}
        if date:
            params["date"] = date.strip()
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SentimentResponse.model_validate(resp.json())

    async def batch_sentiment(
        self,
        tickers: Union[str, list[str]],
        date: Optional[str] = None,
        as_of_utc: Optional[str] = None,
    ) -> BatchSentimentResponse:
        """
        Query point-in-time financial sentiment for multiple stock tickers in a single batch request asynchronously.

        Args:
            tickers: Comma-separated string (e.g. 'AAPL,MSFT,NVDA') or list of ticker symbols (max 50).
            date: Optional historical date string formatted as YYYY-MM-DD or 'LATEST'.
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            BatchSentimentResponse with count and list of SentimentResponse items.
        """
        if isinstance(tickers, list):
            tickers_str = ",".join(str(t).strip().upper() for t in tickers if str(t).strip())
        else:
            tickers_str = str(tickers).strip().upper()

        if not tickers_str:
            raise FinTextValidationError("Field 'tickers' cannot be empty.")

        await self._ensure_authenticated()

        params = {"tickers": tickers_str}
        if date:
            params["date"] = date.strip()
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/batch", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return BatchSentimentResponse.model_validate(resp.json())

    async def sentiment_history(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        limit: int = 100,
        offset: int = 0,
        sort: str = "asc",
        min_quality: Optional[float] = None,
        as_of_utc: Optional[str] = None,
    ) -> SentimentHistoryResponse:
        """
        Query historical sentiment time series for a stock ticker asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            start_date: Start date formatted as YYYY-MM-DD (e.g. '2025-01-01').
            end_date: End date formatted as YYYY-MM-DD (e.g. '2025-03-31').
            limit: Number of records per page (default: 100, max: 1000).
            offset: Number of records to skip for pagination (default: 0).
            sort: Sort direction by timestamp ('asc' or 'desc', default: 'asc').
            min_quality: Minimum data quality score filter threshold (0.0 to 1.0).
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            SentimentHistoryResponse with paginated list of historical sentiment records.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' cannot be empty.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "limit": limit,
            "offset": offset,
            "sort": sort.strip().lower(),
        }
        if min_quality is not None:
            params["min_quality"] = min_quality
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/history", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SentimentHistoryResponse.model_validate(resp.json())

    async def sentiment_feed(
        self,
        sector: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_confidence: Optional[float] = None,
        min_quality: Optional[float] = None,
        limit: int = 100,
        offset: Optional[int] = None,
        cursor: Optional[str] = None,
        sort: str = "desc",
        as_of_utc: Optional[str] = None,
    ) -> SentimentFeedResponse:
        """
        Retrieve a consolidated stream of sentiment signals across multiple tickers and sectors asynchronously.

        Args:
            sector: Optional GICS Sector filter (e.g. 'Technology', 'Financials', 'Healthcare').
            start_date: Earliest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: last 24h).
            end_date: Latest timestamp in ISO format YYYY-MM-DD or RFC3339 (default: now).
            min_confidence: Minimum prediction confidence filter (0.0 to 1.0, default: 0.0).
            min_quality: Minimum data quality score filter (0.0 to 1.0, default: 0.0).
            limit: Maximum number of records per page (default: 100, max: 1000).
            offset: Number of records to skip for pagination (default: 0) [Deprecated: use cursor].
            cursor: Optional cursor for keyset pagination (RFC3339 timestamp of last item from previous page).
            sort: Sort direction by timestamp ('asc' or 'desc', default: 'desc').
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            SentimentFeedResponse with paginated list of consolidated sentiment items.
        """
        if sort.strip().lower() not in ("asc", "desc"):
            raise FinTextValidationError(f"Invalid sort order '{sort}', expected 'asc' or 'desc'.")
        if min_confidence is not None and not (0.0 <= min_confidence <= 1.0):
            raise FinTextValidationError(f"min_confidence must be between 0.0 and 1.0, got {min_confidence}.")
        if min_quality is not None and not (0.0 <= min_quality <= 1.0):
            raise FinTextValidationError(f"min_quality must be between 0.0 and 1.0, got {min_quality}.")
        if limit <= 0 or limit > 1000:
            raise FinTextValidationError(f"limit must be between 1 and 1000, got {limit}.")
        if cursor is not None and offset is not None:
            raise FinTextValidationError("Cannot specify both 'cursor' and 'offset'. Use cursor-based keyset pagination or legacy offset pagination.")
        if cursor is not None and not cursor.strip():
            raise FinTextValidationError("cursor cannot be empty. Expected RFC3339 timestamp string.")
        if offset is not None and offset < 0:
            raise FinTextValidationError(f"offset must be non-negative, got {offset}.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "limit": limit,
            "sort": sort.strip().lower(),
        }
        if cursor is not None:
            params["cursor"] = cursor.strip()
        else:
            params["offset"] = offset if offset is not None else 0
        if sector and sector.strip():
            params["sector"] = sector.strip()
        if start_date and start_date.strip():
            params["start_date"] = start_date.strip()
        if end_date and end_date.strip():
            params["end_date"] = end_date.strip()
        if min_confidence is not None:
            params["min_confidence"] = min_confidence
        if min_quality is not None:
            params["min_quality"] = min_quality
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/feed", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SentimentFeedResponse.model_validate(resp.json())

    async def sentiment_anomalies(
        self,
        sector: Optional[str] = None,
        lookback_days: int = 30,
        zscore_threshold: float = 2.0,
        min_records: int = 20,
        limit: int = 20,
        as_of_utc: Optional[str] = None,
    ) -> SentimentAnomaliesResponse:
        """
        Scan and detect statistically significant sentiment anomalies across universe or sector tickers asynchronously.

        Args:
            sector: Optional GICS Sector filter (e.g. 'Technology', 'Financials', 'Healthcare').
            lookback_days: Number of days to compute baseline statistics (1 to 90, default: 30).
            zscore_threshold: Minimum absolute z-score required to flag anomaly (1.0 to 5.0, default: 2.0).
            min_records: Minimum historical records required per ticker (1 to 1000, default: 20).
            limit: Maximum number of anomaly items to return (1 to 100, default: 20).
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            SentimentAnomaliesResponse with detected and ranked sentiment anomaly items.
        """
        if not (1 <= lookback_days <= 90):
            raise FinTextValidationError(f"lookback_days must be between 1 and 90, got {lookback_days}.")
        if not (1.0 <= zscore_threshold <= 5.0):
            raise FinTextValidationError(f"zscore_threshold must be between 1.0 and 5.0, got {zscore_threshold}.")
        if not (1 <= min_records <= 1000):
            raise FinTextValidationError(f"min_records must be between 1 and 1000, got {min_records}.")
        if not (1 <= limit <= 100):
            raise FinTextValidationError(f"limit must be between 1 and 100, got {limit}.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "lookback_days": lookback_days,
            "zscore_threshold": zscore_threshold,
            "min_records": min_records,
            "limit": limit,
        }
        if sector and sector.strip():
            params["sector"] = sector.strip()
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/anomalies", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SentimentAnomaliesResponse.model_validate(resp.json())

    async def transcribe_audio(
        self,
        audio_file: Union[str, Path, bytes, BinaryIO],
        filename: Optional[str] = None,
        store: bool = False,
        ticker: Optional[str] = None,
        quarter: Optional[int] = None,
        year: Optional[int] = None,
        call_date: Optional[str] = None,
    ) -> AudioTranscriptionResponse:
        """
        Upload an audio recording (WAV, MP3, FLAC, M4A, AAC, OGG up to 50 MB)
        for automated speech-to-text transcription, acoustic vocal stress analysis,
        and financial sentiment classification asynchronously.

        Args:
            audio_file: File path string, Path object, raw bytes, or binary file stream.
            filename: Optional custom filename (e.g. 'earnings_call.wav').
            store: If True, automatically stores transcription in the transcript database.
            ticker: Optional ticker symbol for transcript metadata.
            quarter: Optional fiscal quarter (1-4).
            year: Optional fiscal year (e.g. 2024).
            call_date: Optional call date (YYYY-MM-DD).

        Returns:
            AudioTranscriptionResponse containing transcription, acoustic metrics, sentiment, and optional transcript_id.
        """
        await self._ensure_authenticated()

        file_bytes: bytes
        target_filename: str

        if isinstance(audio_file, (str, Path)):
            path = Path(audio_file)
            if not path.exists():
                raise FinTextValidationError(f"Audio file not found at '{path}'.")
            target_filename = filename or path.name
            file_bytes = path.read_bytes()
        elif isinstance(audio_file, bytes):
            target_filename = filename or "upload.wav"
            file_bytes = audio_file
        elif hasattr(audio_file, "read"):
            target_filename = filename or getattr(audio_file, "name", "upload.wav")
            file_bytes = audio_file.read()
        else:
            raise FinTextValidationError(f"Unsupported audio_file type: {type(audio_file)}")

        if len(file_bytes) > 50 * 1024 * 1024:
            raise FinTextValidationError(f"Audio file size ({len(file_bytes)} bytes) exceeds maximum limit of 50 MB.")

        lower_name = target_filename.lower()
        if lower_name.endswith(".wav"):
            content_type = "audio/wav"
        elif lower_name.endswith(".mp3"):
            content_type = "audio/mpeg"
        elif lower_name.endswith(".flac"):
            content_type = "audio/flac"
        elif lower_name.endswith(".m4a"):
            content_type = "audio/mp4"
        elif lower_name.endswith(".ogg"):
            content_type = "audio/ogg"
        elif lower_name.endswith(".aac"):
            content_type = "audio/aac"
        else:
            content_type = "application/octet-stream"

        files = {"audio": (target_filename, file_bytes, content_type)}
        data: dict[str, Any] = {}
        if store:
            data["store"] = "true"
        if ticker:
            data["ticker"] = ticker.strip().upper()
        if quarter is not None:
            data["quarter"] = str(quarter)
        if year is not None:
            data["year"] = str(year)
        if call_date:
            data["call_date"] = call_date.strip()

        params: dict[str, Any] = {}
        if store:
            params["store"] = "true"
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if quarter is not None:
            params["quarter"] = quarter
        if year is not None:
            params["year"] = year
        if call_date:
            params["call_date"] = call_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post(
                "/audio/transcribe",
                files=files,
                data=data if data else None,
                params=params if params else None,
                headers=headers,
            )
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return AudioTranscriptionResponse.model_validate(resp.json())

    async def export_csv(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        limit: int = 10000,
        min_quality: Optional[float] = None,
    ) -> str:
        """
        Export historical financial sentiment records as a streamed CSV string asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            start_date: Start date formatted as YYYY-MM-DD.
            end_date: End date formatted as YYYY-MM-DD.
            limit: Maximum number of rows to export (default: 10000).
            min_quality: Optional minimum data quality score filter (0.0 to 1.0).

        Returns:
            CSV string content containing header and data rows.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' cannot be empty.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "limit": limit,
        }
        if min_quality is not None:
            params["min_quality"] = min_quality

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/export/csv", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return resp.text

    async def export_parquet(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        limit: int = 10000,
        min_quality: Optional[float] = None,
        include_prices: bool = False,
        output_path: Optional[Union[str, Path]] = None,
    ) -> bytes:
        """
        Export historical financial sentiment records (and optionally daily close prices)
        as a streamed Apache Parquet binary byte buffer asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            start_date: Start date formatted as YYYY-MM-DD.
            end_date: End date formatted as YYYY-MM-DD.
            limit: Maximum number of rows to export (1 to 100000, default: 10000).
            min_quality: Optional minimum data quality score filter (0.0 to 1.0).
            include_prices: Whether to left-join daily closing stock prices (default: False).
            output_path: Optional local file path to save the downloaded Parquet file.

        Returns:
            Raw Apache Parquet file bytes.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' cannot be empty.")
        if limit < 1 or limit > 100000:
            raise FinTextValidationError("Field 'limit' must be between 1 and 100000.")
        if min_quality is not None and (min_quality < 0.0 or min_quality > 1.0):
            raise FinTextValidationError("Field 'min_quality' must be between 0.0 and 1.0.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "limit": limit,
            "include_prices": include_prices,
        }
        if min_quality is not None:
            params["min_quality"] = min_quality

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/export/parquet", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        content = resp.content

        if output_path is not None:
            path_obj = Path(output_path)
            path_obj.parent.mkdir(parents=True, exist_ok=True)
            path_obj.write_bytes(content)

        return content

    async def sector_sentiment(
        self,
        sector: str,
        start_date: str,
        end_date: str,
        aggregation: str = "average",
        min_confidence: float = 0.0,
        as_of_utc: Optional[str] = None,
    ) -> SectorSentimentResponse:
        """
        Query aggregate financial sentiment metrics for a GICS sector over a date range asynchronously.

        Args:
            sector: GICS Sector name (e.g. 'Technology', 'Financials', 'Healthcare', 'Energy').
            start_date: Start date string formatted as YYYY-MM-DD.
            end_date: End date string formatted as YYYY-MM-DD.
            aggregation: Aggregation method ('average', 'sum', 'count', 'median', 'weighted_average').
            min_confidence: Minimum confidence filter between 0.0 and 1.0 (default 0.0).
            as_of_utc: Optional point-in-time ISO-8601 UTC timestamp for SCD2 historical revision filtering.

        Returns:
            SectorSentimentResponse with aggregated value, record count, and constituent tickers count.
        """
        if not sector or not sector.strip():
            raise FinTextValidationError("Field 'sector' cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' cannot be empty.")

        await self._ensure_authenticated()

        params = {
            "sector": sector.strip(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "aggregation": aggregation.strip().lower(),
            "min_confidence": min_confidence,
        }
        if as_of_utc:
            params["as_of_utc"] = as_of_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/sector", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SectorSentimentResponse.model_validate(resp.json())

    async def create_sentiment_revision(
        self,
        ticker: str,
        title: str,
        sentiment_score: float,
        source: Optional[str] = "SEC EDGAR",
        source_id: Optional[str] = None,
        ingested_utc: Optional[str] = None,
        db_commit_utc: Optional[str] = None,
    ) -> RevisionIngestResponse:
        """
        Apply Slowly Changing Dimension Type 2 (SCD2) revision to a sentiment signal asynchronously.

        Supersedes the active sentiment version and registers a new revision version
        with incremented revision_number, setting valid_to on the prior version and
        valid_from on the new version.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            title: Revised headline title or amendment filing summary.
            sentiment_score: Revised sentiment score between -1.0 and 1.0.
            source: Originating source or wire service (default: 'SEC EDGAR').
            source_id: Optional natural key or filing accession identifier.
            ingested_utc: Optional ingestion timestamp in ISO-8601 UTC.
            db_commit_utc: Optional database commit timestamp in ISO-8601 UTC.

        Returns:
            RevisionIngestResponse with active_revision and superseded_revision.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not title or not title.strip():
            raise FinTextValidationError("Field 'title' cannot be empty.")
        if not (-1.0 <= sentiment_score <= 1.0):
            raise FinTextValidationError(f"sentiment_score must be between -1.0 and 1.0, got {sentiment_score}.")

        await self._ensure_authenticated()

        payload: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "title": title.strip(),
            "sentiment_score": sentiment_score,
            "source": source,
        }
        if source_id:
            payload["source_id"] = source_id.strip()
        if ingested_utc:
            payload["ingested_utc"] = ingested_utc.strip()
        if db_commit_utc:
            payload["db_commit_utc"] = db_commit_utc.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/sentiment/revision", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RevisionIngestResponse.model_validate(resp.json())

    async def get_sentiment_revisions(
        self,
        ticker: str,
    ) -> RevisionHistoryListResponse:
        """
        Inspect complete SCD Type 2 revision history and lineage for an asset asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'MSFT').

        Returns:
            RevisionHistoryListResponse containing total count and ordered revision list.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")

        await self._ensure_authenticated()

        params = {"ticker": ticker.strip().upper()}
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/revisions", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RevisionHistoryListResponse.model_validate(resp.json())

    async def spillovers(
        self,
        ticker: str,
        limit: int = 10,
        min_correlation: Optional[float] = None,
    ) -> SpilloverResponse:
        """
        Query cross-asset lead-lag information spillovers asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL').
            limit: Maximum number of related asset spillovers to return (default: 10).
            min_correlation: Optional minimum correlation filter.

        Returns:
            SpilloverResponse with ranked list of correlated assets and lead-lag relationships.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "limit": max(1, min(limit, 50)),
        }
        if min_correlation is not None:
            params["min_correlation"] = min_correlation

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/spillovers", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SpilloverResponse.model_validate(resp.json())

    async def spillover_matrix(
        self,
        start_date: str,
        end_date: str,
        tickers: Optional[Union[list[str], str]] = None,
        min_correlation: float = 0.0,
        max_lag_hours: int = 24,
    ) -> SpilloverMatrixResponse:
        """
        Query cross-asset lead-lag information spillover correlation matrix across multiple tickers asynchronously.

        Args:
            start_date: Analysis start date (YYYY-MM-DD).
            end_date: Analysis end date (YYYY-MM-DD).
            tickers: Optional list or comma-separated string of stock tickers (defaults to core universe).
            min_correlation: Optional minimum absolute correlation filter (-1.0 to 1.0, default: 0.0).
            max_lag_hours: Optional maximum lead-lag window in hours (default: 24, max: 168).

        Returns:
            SpilloverMatrixResponse with pairwise lead-lag correlation matrix items.
        """
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' cannot be empty.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "min_correlation": min_correlation,
            "max_lag_hours": max_lag_hours,
        }

        if tickers:
            if isinstance(tickers, (list, tuple)):
                params["tickers"] = ",".join(t.strip().upper() for t in tickers if t.strip())
            else:
                params["tickers"] = tickers.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/spillovers/matrix", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SpilloverMatrixResponse.model_validate(resp.json())

    async def options_iv(
        self,
        ticker: str,
        expiration_date: str,
        option_type: str = "all",
        strike: Optional[float] = None,
        risk_free_rate: float = 0.05,
        dividend_yield: float = 0.0,
    ) -> OptionsIvResponse:
        """
        Query options implied volatility (IV) and analytical Black-Scholes Greeks (Delta, Gamma, Theta, Vega, Rho) asynchronously.

        Args:
            ticker: Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA').
            expiration_date: Option contract expiration date in ISO YYYY-MM-DD format.
            option_type: Option contract filter ('call', 'put', or 'all', default: 'all').
            strike: Optional specific strike price (if omitted, returns a chain around ATM).
            risk_free_rate: Annualized continuously compounded risk-free rate (default: 0.05).
            dividend_yield: Annualized continuous dividend yield (default: 0.0).

        Returns:
            OptionsIvResponse containing the underlying spot price and list of option contracts with IV and Greeks.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not expiration_date or not expiration_date.strip():
            raise FinTextValidationError("Field 'expiration_date' cannot be empty.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "expiration_date": expiration_date.strip(),
            "option_type": option_type.strip().lower() if option_type else "all",
            "risk_free_rate": risk_free_rate,
            "dividend_yield": dividend_yield,
        }
        if strike is not None:
            if strike <= 0.0:
                raise FinTextValidationError("Field 'strike' must be a positive number.")
            params["strike"] = strike

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/options/iv", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return OptionsIvResponse.model_validate(resp.json())

    async def unusual_options(
        self,
        ticker: Optional[str] = None,
        min_volume_oi_ratio: float = 2.0,
        min_volume: int = 100,
        days: int = 1,
        limit: int = 20,
    ) -> UnusualOptionsResponse:
        """
        Scan options universe for unusual activity (high volume relative to open interest) asynchronously.

        Args:
            ticker: Optional underlying equity ticker filter (e.g. 'AAPL'). If omitted, scans universe.
            min_volume_oi_ratio: Minimum volume / open_interest ratio threshold (default: 2.0).
            min_volume: Minimum absolute contract trading volume to consider (default: 100).
            days: Lookback period in trading days for historical volume comparison (1 to 7, default: 1).
            limit: Maximum number of flagged contracts to return (1 to 100, default: 20).

        Returns:
            UnusualOptionsResponse containing ranked list of contracts sorted by composite score.
        """
        if min_volume_oi_ratio < 0.0:
            raise FinTextValidationError("Field 'min_volume_oi_ratio' cannot be negative.")
        if min_volume < 0:
            raise FinTextValidationError("Field 'min_volume' cannot be negative.")
        if days < 1 or days > 7:
            raise FinTextValidationError("Field 'days' must be between 1 and 7.")
        if limit < 1 or limit > 100:
            raise FinTextValidationError("Field 'limit' must be between 1 and 100.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "min_volume_oi_ratio": min_volume_oi_ratio,
            "min_volume": min_volume,
            "days": days,
            "limit": limit,
        }
        if ticker and ticker.strip():
            params["ticker"] = ticker.strip().upper()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/options/unusual", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return UnusualOptionsResponse.model_validate(resp.json())

    async def usage_stats(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        group_by: str = "day",
        limit: int = 100,
    ) -> UsageStatsResponse:
        """
        Query historical API consumption, endpoint breakdown, latency percentiles, and rate limits.

        Parameters:
            start_date: Optional start date in ISO YYYY-MM-DD format (defaults to 30 days ago)
            end_date: Optional end date in ISO YYYY-MM-DD format (defaults to today)
            group_by: Breakdown grouping dimension ("day", "endpoint", "method", "status_code", default: "day")
            limit: Maximum number of breakdown groups to return (1..=1000, default: 100)

        Returns:
            UsageStatsResponse with summary aggregates and dimensional breakdown.

        Raises:
            FinTextValidationError: If parameters are invalid.
            FinTextAuthError: If unauthorized.
            FinTextRateLimitError: If rate limit exceeded.
            FinTextAPIError: If API returns an error.
        """
        await self._ensure_authenticated()

        group_by_clean = (group_by or "day").strip().lower()
        allowed_groups = {"day", "endpoint", "method", "status_code"}
        if group_by_clean not in allowed_groups:
            raise FinTextValidationError(
                f"Invalid group_by '{group_by}'. Allowed values are: {', '.join(sorted(allowed_groups))}"
            )

        if limit < 1 or limit > 1000:
            raise FinTextValidationError("limit must be between 1 and 1000")

        params: dict[str, Any] = {
            "group_by": group_by_clean,
            "limit": limit,
        }

        if start_date and start_date.strip():
            params["start_date"] = start_date.strip()

        if end_date and end_date.strip():
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/usage/stats", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return UsageStatsResponse.model_validate(resp.json())

    async def event_study(
        self,
        ticker: str,
        event_date: str,
        event_window: int = 5,
        estimation_window: int = 60,
        benchmark_ticker: str = "SPY",
    ) -> EventStudyResponse:
        """
        Execute an Event Study measuring Cumulative Abnormal Returns (CAR) around corporate events asynchronously.

        Parameters:
            ticker: Target equity ticker symbol (e.g., 'AAPL', 'NVDA').
            event_date: Date of the corporate event in ISO YYYY-MM-DD format (e.g., '2025-06-15').
            event_window: Trading days before and after event date (1..=20, default: 5).
            estimation_window: Trading days before event window for OLS market model (10..=120, default: 60).
            benchmark_ticker: Market model benchmark ticker symbol (default: 'SPY').

        Returns:
            EventStudyResponse with OLS alpha, beta, R^2, and daily abnormal returns & CAR trajectory.

        Raises:
            FinTextValidationError: If parameters are invalid or out of bounds.
            FinTextAuthError: If unauthorized.
            FinTextRateLimitError: If rate limit exceeded.
            FinTextAPIError: If API returns an error (e.g. delisted security PIT rejection).
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not event_date or not event_date.strip():
            raise FinTextValidationError("Field 'event_date' cannot be empty.")
        if event_window < 1 or event_window > 20:
            raise FinTextValidationError("Field 'event_window' must be between 1 and 20.")
        if estimation_window < 10 or estimation_window > 120:
            raise FinTextValidationError("Field 'estimation_window' must be between 10 and 120.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "event_date": event_date.strip(),
            "event_window": event_window,
            "estimation_window": estimation_window,
            "benchmark_ticker": (benchmark_ticker or "SPY").strip().upper(),
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/study", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return EventStudyResponse.model_validate(resp.json())

    async def recent_8k(
        self,
        ticker: Optional[str] = None,
        event_type: Optional[str] = None,
        days: int = 7,
        limit: int = 50,
    ) -> EightKResponse:
        """
        Query recent SEC Form 8-K unscheduled corporate disclosure events and classifications asynchronously.

        Parameters:
            ticker: Optional underlying equity ticker symbol filter (e.g., 'AAPL', 'NVDA').
                    If omitted, returns filings across all tracked equities.
            event_type: Optional event type category filter (e.g., 'M&A', 'CEO Change', 'Earnings Warning').
            days: Lookback window in calendar days (1..=30, default: 7).
            limit: Maximum number of filings to return (1..=500, default: 50).

        Returns:
            EightKResponse with total filing count and structured array of EightKFiling items.

        Raises:
            FinTextValidationError: If parameters are invalid or out of bounds.
            FinTextAuthError: If unauthorized.
            FinTextRateLimitError: If rate limit exceeded.
            FinTextAPIError: If API returns an error (e.g., delisted security PIT rejection).
        """
        if days < 1 or days > 30:
            raise FinTextValidationError("Field 'days' must be between 1 and 30.")
        if limit < 1 or limit > 500:
            raise FinTextValidationError("Field 'limit' must be between 1 and 500.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "days": days,
            "limit": limit,
        }

        if ticker is not None:
            if not ticker.strip():
                raise FinTextValidationError("Field 'ticker' cannot be empty.")
            params["ticker"] = ticker.strip().upper()

        if event_type is not None:
            if not event_type.strip():
                raise FinTextValidationError("Field 'event_type' cannot be empty.")
            params["event_type"] = event_type.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/8k", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return EightKResponse.model_validate(resp.json())

    async def supply_chain_risk(
        self,
        ticker: str,
        max_depth: int = 2,
        decay_factor: float = 0.6,
        event_lookback_days: int = 7,
        relationship: Optional[str] = None,
    ) -> SupplyChainRiskResponse:
        """
        Evaluate upstream and downstream supply chain risk propagation and graph intelligence asynchronously.

        Parameters:
            ticker: Root entity stock ticker symbol (e.g., 'AAPL', 'NVDA').
            max_depth: Maximum graph hop traversal depth (1..=4, default: 2).
            decay_factor: Distance decay damping factor applied per hop (0.0..=1.0, default: 0.6).
            event_lookback_days: Lookback window in days for corporate 8-K disclosure events (1..=30, default: 7).
            relationship: Optional relationship category filter ('supplier', 'customer', 'partner', 'competitor').

        Returns:
            SupplyChainRiskResponse with composite risk score, risk tier, and connected node risk profiles.

        Raises:
            FinTextValidationError: If parameters are invalid or out of bounds.
            FinTextAuthError: If unauthorized.
            FinTextRateLimitError: If rate limit exceeded.
            FinTextAPIError: If API returns an error (e.g., delisted security PIT rejection).
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if max_depth < 1 or max_depth > 4:
            raise FinTextValidationError("Field 'max_depth' must be between 1 and 4.")
        if decay_factor < 0.0 or decay_factor > 1.0:
            raise FinTextValidationError("Field 'decay_factor' must be between 0.0 and 1.0.")
        if event_lookback_days < 1 or event_lookback_days > 30:
            raise FinTextValidationError("Field 'event_lookback_days' must be between 1 and 30.")

        self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "max_depth": max_depth,
            "decay_factor": decay_factor,
            "event_lookback_days": event_lookback_days,
        }

        if relationship is not None:
            if not relationship.strip():
                raise FinTextValidationError("Field 'relationship' cannot be empty.")
            valid_rels = {"supplier", "customer", "partner", "competitor"}
            rel_lower = relationship.strip().lower()
            if rel_lower not in valid_rels:
                raise FinTextValidationError(f"Field 'relationship' must be one of {sorted(valid_rels)}.")
            params["relationship"] = rel_lower

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/risk/supply-chain", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SupplyChainRiskResponse.model_validate(resp.json())

    async def backtest(
        self,
        request: Optional[Union[BacktestRequest, dict[str, Any]]] = None,
        *,
        ticker: Optional[str] = None,
        tickers: Optional[list[str]] = None,
        weights: Optional[list[float]] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        long_threshold: float = 0.2,
        short_threshold: float = -0.2,
        holding_days: int = 5,
        initial_capital: float = 1_000_000.0,
        transaction_cost_bps: float = 5.0,
        benchmark_ticker: Optional[str] = "SPY",
    ) -> BacktestResponse:
        """
        Execute point-in-time quantitative alpha strategy backtest asynchronously.

        Args:
            request: Optional BacktestRequest object or dictionary of simulation parameters.
            ticker: Single target asset ticker symbol.
            tickers: Multi-asset portfolio tickers (1 to 10).
            weights: Portfolio weights per ticker.
            start_date: Simulation start date (YYYY-MM-DD).
            end_date: Simulation end date (YYYY-MM-DD).
            long_threshold: Sentiment score to enter LONG.
            short_threshold: Sentiment score to enter SHORT.
            holding_days: Holding duration in trading days.
            initial_capital: Starting cash capital in USD.
            transaction_cost_bps: Transaction fee in basis points.
            benchmark_ticker: Benchmark asset ticker.

        Returns:
            BacktestResponse with total returns, Sharpe ratio, drawdown, and equity curve.
        """
        await self._ensure_authenticated()

        if request is not None:
            if isinstance(request, dict):
                req_model = BacktestRequest.model_validate(request)
            elif isinstance(request, BacktestRequest):
                req_model = request
            else:
                raise FinTextValidationError("Request must be a BacktestRequest or dict.")
        else:
            if not start_date or not end_date:
                raise FinTextValidationError("Fields 'start_date' and 'end_date' are required.")
            if not ticker and not tickers:
                raise FinTextValidationError("Either 'ticker' or 'tickers' must be specified.")
            
            req_model = BacktestRequest(
                ticker=ticker,
                tickers=tickers,
                weights=weights,
                start_date=start_date,
                end_date=end_date,
                long_threshold=long_threshold,
                short_threshold=short_threshold,
                holding_days=holding_days,
                initial_capital=initial_capital,
                transaction_cost_bps=transaction_cost_bps,
                benchmark_ticker=benchmark_ticker,
            )

        payload = req_model.model_dump(exclude_none=True)
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/backtest", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return BacktestResponse.model_validate(resp.json())

    async def symbol_map(
        self,
        identifier: str,
        input_type: str = "auto",
        output_type: str = "all",
    ) -> SymbolMapResponse:
        """
        Resolve financial security identifiers across Ticker, FIGI, CUSIP, and ISIN asynchronously.

        Args:
            identifier: Security identifier to resolve (e.g. 'AAPL', 'BBG000B9XRY4', '037833100', 'US0378331005').
            input_type: Input identifier scheme ('auto', 'ticker', 'figi', 'cusip', 'isin', default: 'auto').
            output_type: Desired output identifier format ('ticker', 'figi', 'cusip', 'isin', 'all', default: 'all').

        Returns:
            SymbolMapResponse with resolved security identifiers.
        """
        raw_id = identifier.strip()
        if not raw_id:
            raise FinTextValidationError("Field 'identifier' cannot be empty.")

        await self._ensure_authenticated()

        params = {
            "identifier": raw_id,
            "input_type": input_type.strip().lower(),
            "output_type": output_type.strip().lower(),
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/symbols/map", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SymbolMapResponse.model_validate(resp.json())

    async def batch_sentiment(
        self,
        tickers: Optional[Union[list[str], str]] = None,
        universe_id: Optional[str] = None,
        date: Optional[str] = None,
    ) -> BatchSentimentResponse:
        """
        Query Point-in-Time sentiment signals for multiple tickers or a saved universe asynchronously.

        Args:
            tickers: Optional list or comma-separated string of stock tickers (max 50).
            universe_id: Optional UUID string of a saved custom universe.
            date: Optional historical date formatted as YYYY-MM-DD or 'LATEST'.

        Returns:
            BatchSentimentResponse with individual sentiment signals for each constituent asset.
        """
        if tickers is not None and universe_id is not None:
            raise FinTextValidationError("Specify either tickers or universe_id, not both.")
        if tickers is None and universe_id is None:
            raise FinTextValidationError("Either tickers or universe_id must be provided.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {}
        if tickers is not None:
            if isinstance(tickers, list):
                raw_tickers = ",".join(s.strip().upper() for s in tickers if s.strip())
            elif isinstance(tickers, str):
                raw_tickers = tickers.strip().upper()
            else:
                raise FinTextValidationError("Field 'tickers' must be a list of strings or a comma-separated string.")

            if not raw_tickers:
                raise FinTextValidationError("Field 'tickers' cannot be empty.")
            params["tickers"] = raw_tickers
        elif universe_id is not None:
            raw_uid = universe_id.strip()
            if not raw_uid:
                raise FinTextValidationError("Field 'universe_id' cannot be empty.")
            params["universe_id"] = raw_uid

        if date and date.strip():
            params["date"] = date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/batch", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return BatchSentimentResponse.model_validate(resp.json())

    async def create_universe(
        self,
        name: str,
        tickers: list[str],
    ) -> Universe:
        """
        Create a new custom security universe (watchlist) asynchronously.

        Args:
            name: Universe display name (1-100 characters).
            tickers: List of constituent stock tickers (1-100 tickers).

        Returns:
            Universe object with assigned UUID and creation timestamps.
        """
        if not name or not name.strip():
            raise FinTextValidationError("Field 'name' cannot be empty.")
        if not tickers:
            raise FinTextValidationError("Field 'tickers' cannot be empty.")

        await self._ensure_authenticated()

        payload = {
            "name": name.strip(),
            "tickers": [t.strip().upper() for t in tickers if t.strip()],
        }
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/universes", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return Universe.model_validate(resp.json())

    async def list_universes(
        self,
        limit: int = 100,
        offset: int = 0,
    ) -> ListUniversesResponse:
        """
        List custom universes defined by the authenticated user asynchronously.

        Args:
            limit: Number of universes to return per page (default: 100, max: 1000).
            offset: Number of universes to skip for pagination (default: 0).

        Returns:
            ListUniversesResponse with list of universes.
        """
        await self._ensure_authenticated()

        params = {"limit": limit, "offset": offset}
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/universes", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ListUniversesResponse.model_validate(resp.json())

    async def get_universe(
        self,
        universe_id: str,
    ) -> Universe:
        """
        Get a specific custom universe by UUID asynchronously.

        Args:
            universe_id: Unique Universe identifier UUID.

        Returns:
            Universe object.
        """
        if not universe_id or not universe_id.strip():
            raise FinTextValidationError("Field 'universe_id' cannot be empty.")

        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/universes/{universe_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return Universe.model_validate(resp.json())

    async def update_universe(
        self,
        universe_id: str,
        name: Optional[str] = None,
        tickers: Optional[list[str]] = None,
    ) -> Universe:
        """
        Update an existing custom universe asynchronously.

        Args:
            universe_id: Unique Universe identifier UUID.
            name: Optional updated universe display name.
            tickers: Optional updated list of ticker symbols.

        Returns:
            Updated Universe object.
        """
        if not universe_id or not universe_id.strip():
            raise FinTextValidationError("Field 'universe_id' cannot be empty.")

        await self._ensure_authenticated()

        payload: dict[str, Any] = {}
        if name is not None:
            payload["name"] = name.strip()
        if tickers is not None:
            payload["tickers"] = [t.strip().upper() for t in tickers if t.strip()]

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.put(f"/universes/{universe_id.strip()}", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return Universe.model_validate(resp.json())

    async def delete_universe(
        self,
        universe_id: str,
    ) -> DeleteUniverseResponse:
        """
        Delete a custom universe by UUID asynchronously.

        Args:
            universe_id: Unique Universe identifier UUID.

        Returns:
            DeleteUniverseResponse confirming deletion.
        """
        if not universe_id or not universe_id.strip():
            raise FinTextValidationError("Field 'universe_id' cannot be empty.")

        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/universes/{universe_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeleteUniverseResponse.model_validate(resp.json())

    async def create_transcript(
        self,
        ticker: str,
        transcript_text: str,
        quarter: Optional[int] = None,
        year: Optional[int] = None,
        call_date: Optional[str] = None,
        source: Optional[str] = None,
        sentiment_score: Optional[float] = None,
        sentiment_label: Optional[str] = None,
        confidence: Optional[float] = None,
    ) -> TranscriptResponse:
        """
        Upload and store an earnings call transcript with automated or custom sentiment scoring asynchronously.

        Args:
            ticker: Stock asset ticker symbol (e.g. 'AAPL', 'NVDA').
            transcript_text: Full textual body of the earnings call transcript.
            quarter: Optional fiscal quarter (1-4).
            year: Optional fiscal year (e.g. 2024).
            call_date: Optional call date formatted as YYYY-MM-DD.
            source: Optional data source identifier (default: 'manual').
            sentiment_score: Optional override sentiment score (-1.0 to 1.0).
            sentiment_label: Optional override sentiment label ('BULLISH', 'BEARISH', 'NEUTRAL').
            confidence: Optional override sentiment confidence score (0.0 to 1.0).

        Returns:
            TranscriptResponse containing stored transcript metadata, text, and sentiment scores.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Field 'ticker' cannot be empty.")
        if not transcript_text or not transcript_text.strip():
            raise FinTextValidationError("Field 'transcript_text' cannot be empty.")

        await self._ensure_authenticated()

        payload: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "transcript_text": transcript_text.strip(),
        }
        if quarter is not None:
            payload["quarter"] = quarter
        if year is not None:
            payload["year"] = year
        if call_date:
            payload["call_date"] = call_date.strip()
        if source:
            payload["source"] = source.strip()
        if sentiment_score is not None:
            payload["sentiment_score"] = sentiment_score
        if sentiment_label:
            payload["sentiment_label"] = sentiment_label.strip()
        if confidence is not None:
            payload["confidence"] = confidence

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/transcripts", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return TranscriptResponse.model_validate(resp.json())

    async def list_transcripts(
        self,
        ticker: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        quarter: Optional[int] = None,
        year: Optional[int] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> TranscriptListResponse:
        """
        List earnings call transcript metadata with filtering and pagination asynchronously.

        Args:
            ticker: Optional ticker symbol filter.
            start_date: Optional start date filter (YYYY-MM-DD).
            end_date: Optional end date filter (YYYY-MM-DD).
            quarter: Optional fiscal quarter filter (1-4).
            year: Optional fiscal year filter.
            limit: Maximum items per page (default: 20, max: 100).
            offset: Number of items to skip for pagination (default: 0).

        Returns:
            TranscriptListResponse containing total count and list of TranscriptMetadata records.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "limit": limit,
            "offset": offset,
        }
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()
        if quarter is not None:
            params["quarter"] = quarter
        if year is not None:
            params["year"] = year

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/transcripts", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return TranscriptListResponse.model_validate(resp.json())

    async def get_transcript(
        self,
        transcript_id: str,
    ) -> TranscriptResponse:
        """
        Retrieve a single earnings call transcript with full text by UUID asynchronously.

        Args:
            transcript_id: Unique transcript UUID.

        Returns:
            TranscriptResponse containing complete transcript and sentiment analysis.
        """
        if not transcript_id or not transcript_id.strip():
            raise FinTextValidationError("Field 'transcript_id' cannot be empty.")

        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/transcripts/{transcript_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return TranscriptResponse.model_validate(resp.json())

    async def delete_transcript(
        self,
        transcript_id: str,
    ) -> DeleteTranscriptResponse:
        """
        Delete an earnings call transcript by UUID asynchronously.

        Args:
            transcript_id: Unique transcript UUID.

        Returns:
            DeleteTranscriptResponse confirming deletion.
        """
        if not transcript_id or not transcript_id.strip():
            raise FinTextValidationError("Field 'transcript_id' cannot be empty.")

        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/transcripts/{transcript_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeleteTranscriptResponse.model_validate(resp.json())

    async def market_regime(
        self,
        lookback_days: int = 5,
        sector_weights: Optional[str] = None,
        min_data_points: int = 50,
    ) -> MarketRegimeResponse:
        """
        Query macro market regime classification and underlying quantitative components asynchronously.

        Args:
            lookback_days: Number of lookback days for computing aggregate market statistics (default: 5, max: 30).
            sector_weights: Optional sector weighting string (e.g. "Technology:0.4,Financials:0.3") or comma-separated floats.
            min_data_points: Minimum sentiment records required for high statistical confidence (default: 50).

        Returns:
            MarketRegimeResponse with regime classification (Bullish, Bearish, Neutral, High Volatility) and components.
        """
        if lookback_days < 1 or lookback_days > 30:
            raise FinTextValidationError("Field 'lookback_days' must be between 1 and 30.")
        if min_data_points < 1 or min_data_points > 1000:
            raise FinTextValidationError("Field 'min_data_points' must be between 1 and 1000.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "lookback_days": lookback_days,
            "min_data_points": min_data_points,
        }
        if sector_weights:
            params["sector_weights"] = sector_weights.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/market/regime", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return MarketRegimeResponse.model_validate(resp.json())

    async def return_correlation(
        self,
        tickers: Union[str, list[str]],
        start_date: str,
        end_date: str,
        min_periods: int = 20,
        include_self: bool = False,
    ) -> ReturnCorrelationResponse:
        """
        Compute the Pearson correlation matrix of daily stock returns across a universe asynchronously.

        Args:
            tickers: Comma-separated string or list of ticker symbols (max 50, e.g. "AAPL,MSFT,NVDA" or ["AAPL", "MSFT"]).
            start_date: Start date for correlation window (YYYY-MM-DD).
            end_date: End date for correlation window (YYYY-MM-DD).
            min_periods: Minimum overlapping return periods required for valid correlation (default: 20, min: 10).
            include_self: Whether to include self-correlation pairs (1.0) in the matrix (default: False).

        Returns:
            ReturnCorrelationResponse with pairwise correlation metrics and sample periods.
        """
        if isinstance(tickers, list):
            ticker_str = ",".join(t.strip().upper() for t in tickers if t.strip())
        else:
            ticker_str = tickers.strip()

        if not ticker_str:
            raise FinTextValidationError("Parameter 'tickers' must not be empty.")
        if not start_date.strip():
            raise FinTextValidationError("Parameter 'start_date' must not be empty.")
        if not end_date.strip():
            raise FinTextValidationError("Parameter 'end_date' must not be empty.")
        if min_periods < 10 or min_periods > 1000:
            raise FinTextValidationError("Field 'min_periods' must be between 10 and 1000.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "tickers": ticker_str,
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "min_periods": min_periods,
            "include_self": "true" if include_self else "false",
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/market/correlation", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ReturnCorrelationResponse.model_validate(resp.json())

    async def put_call_ratio(
        self,
        start_date: str,
        end_date: str,
        ticker: Optional[str] = None,
        ratio_type: str = "volume",
        granularity: str = "daily",
    ) -> PutCallRatioResponse:
        """
        Query options put/call volume and open interest ratios for a ticker or market-wide universe asynchronously.

        Args:
            start_date: Start date of observation window (YYYY-MM-DD).
            end_date: End date of observation window (YYYY-MM-DD).
            ticker: Optional underlying equity ticker symbol (e.g., "AAPL"). If omitted, market-wide aggregate ratio is computed.
            ratio_type: Ratio metric to evaluate: "volume" (default) or "open_interest".
            granularity: Aggregation granularity: "daily" (default) or "total".

        Returns:
            PutCallRatioResponse containing daily time series or period aggregate totals.
        """
        if not start_date.strip():
            raise FinTextValidationError("Parameter 'start_date' must not be empty.")
        if not end_date.strip():
            raise FinTextValidationError("Parameter 'end_date' must not be empty.")

        ratio_type_norm = ratio_type.strip().lower()
        if ratio_type_norm not in ("volume", "open_interest"):
            raise FinTextValidationError(f"Invalid ratio_type '{ratio_type}'. Allowed values: 'volume', 'open_interest'.")

        granularity_norm = granularity.strip().lower()
        if granularity_norm not in ("daily", "total"):
            raise FinTextValidationError(f"Invalid granularity '{granularity}'. Allowed values: 'daily', 'total'.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "ratio_type": ratio_type_norm,
            "granularity": granularity_norm,
        }
        if ticker:
            params["ticker"] = ticker.strip().upper()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/options/put-call-ratio", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PutCallRatioResponse.model_validate(resp.json())

    async def earnings_surprise(
        self,
        ticker: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_sentiment_shift: float = 0.15,
        pre_days: int = 5,
        post_days: int = 5,
        limit: int = 20,
    ) -> EarningsSurpriseResponse:
        """
        Query detected earnings surprise events based on pre/post earnings sentiment displacement asynchronously.

        Args:
            ticker: Optional target equity ticker symbol (e.g., "AAPL"). If omitted, scans tracked universe.
            start_date: Optional observation start date (YYYY-MM-DD, defaults to 90 days ago).
            end_date: Optional observation end date (YYYY-MM-DD, defaults to today).
            min_sentiment_shift: Minimum absolute change in average sentiment (0.05 to 0.50, default: 0.15).
            pre_days: Number of trading days before earnings date to compute baseline sentiment (1 to 10, default: 5).
            post_days: Number of trading days on/after earnings date to compute reaction sentiment (1 to 10, default: 5).
            limit: Maximum number of events to return (1 to 100, default: 20).

        Returns:
            EarningsSurpriseResponse containing detected earnings surprises.
        """
        if not (0.05 <= min_sentiment_shift <= 0.50):
            raise FinTextValidationError("Parameter 'min_sentiment_shift' must be between 0.05 and 0.50.")
        if not (1 <= pre_days <= 10):
            raise FinTextValidationError("Parameter 'pre_days' must be between 1 and 10.")
        if not (1 <= post_days <= 10):
            raise FinTextValidationError("Parameter 'post_days' must be between 1 and 10.")
        if not (1 <= limit <= 100):
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 100.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "min_sentiment_shift": min_sentiment_shift,
            "pre_days": pre_days,
            "post_days": post_days,
            "limit": limit,
        }
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/earnings-surprise", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return EarningsSurpriseResponse.model_validate(resp.json())

    async def insider_trading(
        self,
        ticker: Optional[str] = None,
        transaction_type: str = "all",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_shares: int = 0,
        min_signal_score: float = 0.0,
        limit: int = 20,
    ) -> InsiderTradingResponse:
        """
        Query SEC Form 4 insider trading transactions and normalized conviction signal scores asynchronously.

        Args:
            ticker: Optional target equity ticker symbol (e.g., "AAPL"). If omitted, scans tracked universe.
            transaction_type: Transaction type filter ("all", "purchase", "sale", "grant", "exercise").
            start_date: Optional observation start date (YYYY-MM-DD, defaults to 90 days ago).
            end_date: Optional observation end date (YYYY-MM-DD, defaults to today).
            min_shares: Minimum number of shares traded filter (default: 0).
            min_signal_score: Minimum absolute signal strength filter (0.0 to 1.0, default: 0.0).
            limit: Maximum number of transactions to return (1 to 100, default: 20).

        Returns:
            InsiderTradingResponse containing matched insider transactions.
        """
        t_type_norm = transaction_type.strip().lower()
        if t_type_norm not in ("all", "purchase", "sale", "grant", "exercise"):
            raise FinTextValidationError(f"Invalid transaction_type '{transaction_type}'. Allowed: 'all', 'purchase', 'sale', 'grant', 'exercise'.")
        if min_shares < 0:
            raise FinTextValidationError("Parameter 'min_shares' must be >= 0.")
        if not (0.0 <= min_signal_score <= 1.0):
            raise FinTextValidationError("Parameter 'min_signal_score' must be between 0.0 and 1.0.")
        if not (1 <= limit <= 100):
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 100.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "transaction_type": t_type_norm,
            "min_shares": min_shares,
            "min_signal_score": min_signal_score,
            "limit": limit,
        }
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/insider-trading", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return InsiderTradingResponse.model_validate(resp.json())

    async def sentiment_disagreement(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        source: Optional[str] = None,
        min_records: int = 10,
        aggregation: str = "stddev",
    ) -> SentimentDisagreementResponse:
        """
        Query the sentiment disagreement index and multi-source dispersion metrics for a ticker asynchronously.

        Args:
            ticker: Target equity ticker symbol (e.g., "AAPL").
            start_date: Start date of observation window (YYYY-MM-DD).
            end_date: End date of observation window (YYYY-MM-DD).
            source: Optional news/filing source filter (e.g. "SEC EDGAR", "Finnhub").
            min_records: Minimum required sentiment records (default: 10, min: 5).
            aggregation: Dispersion aggregation method ("stddev", "iqr", "mad", default: "stddev").

        Returns:
            SentimentDisagreementResponse containing dispersion index, central tendencies, and source breakdowns.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Parameter 'ticker' is required and cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Parameter 'start_date' is required.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Parameter 'end_date' is required.")
        if min_records < 5:
            raise FinTextValidationError("Parameter 'min_records' must be at least 5.")
        agg_norm = aggregation.strip().lower()
        if agg_norm not in ("stddev", "iqr", "mad"):
            raise FinTextValidationError(f"Invalid aggregation '{aggregation}'. Allowed: 'stddev', 'iqr', 'mad'.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "min_records": min_records,
            "aggregation": agg_norm,
        }
        if source and source.strip():
            params["source"] = source.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sentiment/disagreement", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SentimentDisagreementResponse.model_validate(resp.json())

    async def options_vol_surface(
        self,
        ticker: str,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        strike_range: Optional[str] = "0.8-1.2",
        strike_count: int = 9,
        risk_free_rate: float = 0.05,
        dividend_yield: float = 0.0,
    ) -> OptionsVolSurfaceResponse:
        """
        Query the 2D options implied volatility surface grid across strikes and expirations asynchronously.

        Args:
            ticker: Target underlying equity ticker symbol (e.g. "AAPL", "NVDA").
            start_date: Earliest expiration date (ISO YYYY-MM-DD, default: today).
            end_date: Latest expiration date (ISO YYYY-MM-DD, default: +6 months).
            strike_range: Multiplier range of strikes relative to spot (e.g. "0.8-1.2").
            strike_count: Number of symmetric strikes around ATM (default: 9, odd integer between 3 and 15).
            risk_free_rate: Annualized risk-free interest rate (default: 0.05, 0.0 to 0.20).
            dividend_yield: Annualized continuous dividend yield (default: 0.0, 0.0 to 0.10).

        Returns:
            OptionsVolSurfaceResponse containing 2D volatility matrix, strikes, and expirations.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Parameter 'ticker' is required and cannot be empty.")
        if strike_count < 3 or strike_count > 15 or strike_count % 2 == 0:
            raise FinTextValidationError("Parameter 'strike_count' must be an odd integer between 3 and 15.")
        if risk_free_rate < 0.0 or risk_free_rate > 0.20:
            raise FinTextValidationError("Parameter 'risk_free_rate' must be between 0.0 and 0.20.")
        if dividend_yield < 0.0 or dividend_yield > 0.10:
            raise FinTextValidationError("Parameter 'dividend_yield' must be between 0.0 and 0.10.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "strike_count": strike_count,
            "risk_free_rate": risk_free_rate,
            "dividend_yield": dividend_yield,
        }
        if start_date and start_date.strip():
            params["start_date"] = start_date.strip()
        if end_date and end_date.strip():
            params["end_date"] = end_date.strip()
        if strike_range and strike_range.strip():
            params["strike_range"] = strike_range.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/options/vol-surface", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return OptionsVolSurfaceResponse.model_validate(resp.json())

    async def ma_rumors(
        self,
        ticker: Optional[str] = None,
        min_rumor_score: float = 0.50,
        lookback_days: int = 7,
        limit: int = 20,
    ) -> MARumorsResponse:
        """
        Query M&A rumor detection signals combining sentiment anomalies, keyword hits, 8-K filings,
        supply chain relationships, and insider trading accumulation asynchronously.

        Args:
            ticker: Optional ticker symbol to filter for. If omitted, scans across tracked universe.
            min_rumor_score: Minimum composite rumor score threshold (0.0 to 1.0, default: 0.50).
            lookback_days: Observation window in calendar days (1 to 30, default: 7).
            limit: Maximum number of flagged candidates to return (1 to 100, default: 20).

        Returns:
            MARumorsResponse containing array of flagged M&A rumor items.
        """
        if min_rumor_score < 0.0 or min_rumor_score > 1.0:
            raise FinTextValidationError("Parameter 'min_rumor_score' must be between 0.0 and 1.0.")
        if lookback_days < 1 or lookback_days > 30:
            raise FinTextValidationError("Parameter 'lookback_days' must be between 1 and 30.")
        if limit < 1 or limit > 100:
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 100.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "min_rumor_score": min_rumor_score,
            "lookback_days": lookback_days,
            "limit": limit,
        }
        if ticker and ticker.strip():
            params["ticker"] = ticker.strip().upper()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/ma-rumors", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return MARumorsResponse.model_validate(resp.json())

    async def regulatory_filings(
        self,
        ticker: Optional[str] = None,
        form_type: Optional[str] = None,
        event_category: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> RegulatoryFilingsResponse:
        """
        Retrieve and classify SEC regulatory filings (10-K, 10-Q, 8-K, S-1, 20-F, DEF 14A, etc.)
        with standardized event categories and SEC EDGAR metadata asynchronously.

        Args:
            ticker: Optional ticker symbol to filter for. If omitted, scans across tracked universe.
            form_type: Optional SEC form type filter (e.g. '10-K', '10-Q', '8-K', 'S-1').
            event_category: Optional classified event category filter (e.g. 'Annual Report', 'M&A', 'Earnings').
            start_date: Earliest filing date (ISO YYYY-MM-DD, default: 90 days ago).
            end_date: Latest filing date (ISO YYYY-MM-DD, default: today).
            limit: Maximum number of filings to return (1 to 100, default: 20).
            offset: Number of records to skip for pagination (default: 0).

        Returns:
            RegulatoryFilingsResponse containing array of classified regulatory filings and pagination metadata.
        """
        if limit < 1 or limit > 100:
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 100.")
        if offset < 0:
            raise FinTextValidationError("Parameter 'offset' must be greater than or equal to 0.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "limit": limit,
            "offset": offset,
        }
        if ticker and ticker.strip():
            params["ticker"] = ticker.strip().upper()
        if form_type and form_type.strip():
            params["form_type"] = form_type.strip().upper()
        if event_category and event_category.strip():
            params["event_category"] = event_category.strip()
        if start_date and start_date.strip():
            params["start_date"] = start_date.strip()
        if end_date and end_date.strip():
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/events/filings", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RegulatoryFilingsResponse.model_validate(resp.json())

    async def export_parquet(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        limit: int = 10_000,
        min_quality: float = 0.0,
        include_prices: bool = False,
        output_path: Optional[Any] = None,
    ) -> bytes:
        """
        Export historical sentiment data (and optionally stock prices) in Apache Parquet format asynchronously.

        Args:
            ticker: Target stock ticker symbol (e.g. 'AAPL', 'NVDA').
            start_date: Start date in ISO format YYYY-MM-DD (e.g. '2025-01-01').
            end_date: End date in ISO format YYYY-MM-DD (e.g. '2025-03-31').
            limit: Maximum number of rows to export (1 to 100000, default: 10000).
            min_quality: Minimum data quality score filter (0.0 to 1.0, default: 0.0).
            include_prices: Whether to left-join daily closing stock prices on date (default: False).
            output_path: Optional file path to write the Parquet bytes to disk.

        Returns:
            Raw Parquet binary data as bytes.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Parameter 'ticker' is required and cannot be empty.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Parameter 'start_date' is required and cannot be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Parameter 'end_date' is required and cannot be empty.")
        if limit < 1 or limit > 100_000:
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 100000.")
        if min_quality < 0.0 or min_quality > 1.0:
            raise FinTextValidationError("Parameter 'min_quality' must be between 0.0 and 1.0.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "limit": limit,
            "min_quality": min_quality,
            "include_prices": str(include_prices).lower(),
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/export/parquet", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)

        data = resp.content
        if output_path is not None:
            from pathlib import Path
            Path(output_path).write_bytes(data)

        return data

    async def ws_url(self, ticker: Optional[str] = None, format: str = "json") -> str:
        """
        Generate the authenticated WebSocket connection URL asynchronously.

        Args:
            ticker: Optional ticker to filter real-time stream updates.
            format: Streaming serialization format ("json" or "msgpack"). Defaults to "json".

        Returns:
            Full WebSocket URI with token parameter (e.g. ws://127.0.0.1:8000/ws?token=...&format=msgpack).
        """
        await self._ensure_authenticated()

        parsed = urlparse(self.base_url)
        ws_scheme = "wss" if parsed.scheme == "https" else "ws"
        netloc = parsed.netloc or parsed.path

        query_params = {"token": self.api_token}
        if ticker:
            query_params["ticker"] = ticker.strip().upper()
        if format and format.strip().lower() in ("msgpack", "messagepack", "mp"):
            query_params["format"] = "msgpack"

        return urlunparse((
            ws_scheme,
            netloc,
            "/ws",
            "",
            urlencode(query_params),
            "",
        ))

    # ─────────────────────────────────────────────────────────────────────────
    # Billing & Subscription Management
    # ─────────────────────────────────────────────────────────────────────────

    async def create_checkout_session(
        self,
        plan_id: str,
        *,
        success_url: Optional[str] = None,
        cancel_url: Optional[str] = None,
    ) -> CheckoutResponse:
        """Create a Stripe Checkout Session for subscribing to a paid plan asynchronously.

        Args:
            plan_id: Subscription plan identifier (e.g. 'pro_monthly', 'enterprise_monthly').
            success_url: URL to redirect to after successful payment.
            cancel_url: URL to redirect to if user cancels checkout.

        Returns:
            CheckoutResponse with checkout_url and session_id.

        Raises:
            FinTextValidationError: If the plan_id is invalid.
        """
        if not plan_id or not plan_id.strip():
            raise FinTextValidationError("Field 'plan_id' cannot be empty.")

        await self._ensure_authenticated()

        payload: dict[str, Any] = {"plan_id": plan_id.strip()}
        if success_url:
            payload["success_url"] = success_url
        if cancel_url:
            payload["cancel_url"] = cancel_url

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/billing/checkout", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return CheckoutResponse.model_validate(resp.json())

    async def create_portal_session(
        self,
        *,
        return_url: Optional[str] = None,
    ) -> PortalResponse:
        """Create a Stripe Customer Portal session for subscription management asynchronously.

        Args:
            return_url: URL to return to after leaving the portal.

        Returns:
            PortalResponse with portal_url.
        """
        await self._ensure_authenticated()

        payload: dict[str, Any] = {}
        if return_url:
            payload["return_url"] = return_url

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/billing/portal", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return PortalResponse.model_validate(resp.json())

    async def get_subscription(self) -> SubscriptionResponse:
        """Get the current user's subscription status and usage asynchronously.

        Returns:
            SubscriptionResponse with plan_id, status, limits, and current usage.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/billing/subscription", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return SubscriptionResponse.model_validate(resp.json())

    # ─── Organizations & Teams ─────────────────────────────────────────────

    async def create_org(self, name: str) -> CreateOrgResponse:
        """Create a new organization asynchronously.

        Args:
            name: Display name for the new organization.

        Returns:
            CreateOrgResponse with id, name, role, created_at.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        payload = {"name": name}

        try:
            resp = await self._client.post("/orgs", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return CreateOrgResponse.model_validate(resp.json())

    async def list_orgs(self) -> ListOrgsResponse:
        """List all organizations the current user belongs to asynchronously.

        Returns:
            ListOrgsResponse with organizations array and count.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/orgs", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return ListOrgsResponse.model_validate(resp.json())

    async def get_org(self, org_id: str) -> OrgDetailsResponse:
        """Get organization details including member roster asynchronously.

        Args:
            org_id: UUID of the organization.

        Returns:
            OrgDetailsResponse with members list.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/orgs/{org_id}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return OrgDetailsResponse.model_validate(resp.json())

    async def invite_member(
        self, org_id: str, user_id: str, role: str = "member"
    ) -> InviteMemberResponse:
        """Invite or add a user to an organization asynchronously.

        Args:
            org_id: UUID of the organization.
            user_id: Identifier of the user to invite.
            role: Role to assign (admin, member, viewer). Defaults to 'member'.

        Returns:
            InviteMemberResponse with status and assigned role.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        payload = {"user_id": user_id, "role": role}

        try:
            resp = await self._client.post(f"/orgs/{org_id}/invites", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return InviteMemberResponse.model_validate(resp.json())

    async def update_member_role(
        self, org_id: str, user_id: str, role: str
    ) -> UpdateMemberRoleResponse:
        """Update a member's role within an organization asynchronously.

        Args:
            org_id: UUID of the organization.
            user_id: Identifier of the member to update.
            role: New role to assign (admin, member, viewer).

        Returns:
            UpdateMemberRoleResponse with status and new role.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        payload = {"role": role}

        try:
            resp = await self._client.patch(f"/orgs/{org_id}/members/{user_id}", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return UpdateMemberRoleResponse.model_validate(resp.json())

    async def remove_member(self, org_id: str, user_id: str) -> RemoveMemberResponse:
        """Remove a member from an organization asynchronously.

        Args:
            org_id: UUID of the organization.
            user_id: Identifier of the member to remove.

        Returns:
            RemoveMemberResponse with status.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/orgs/{org_id}/members/{user_id}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return RemoveMemberResponse.model_validate(resp.json())

    async def leave_org(self, org_id: str) -> LeaveOrgResponse:
        """Leave an organization asynchronously.

        Args:
            org_id: UUID of the organization to leave.

        Returns:
            LeaveOrgResponse with status.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post(f"/orgs/{org_id}/leave", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return LeaveOrgResponse.model_validate(resp.json())

    async def select_org(self, org_id: str) -> SelectOrgResponse:
        """Select an organization as the active context asynchronously.

        Args:
            org_id: UUID of the organization to activate.

        Returns:
            SelectOrgResponse with new token, org_id, and role.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post(f"/orgs/{org_id}/select", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        result = SelectOrgResponse.model_validate(resp.json())
        # Auto-update the client's token with the org-scoped JWT
        self.api_token = result.token
        return result

    # ─── Security & IP Whitelisting ──────────────────────────────────────────

    async def get_ip_whitelist(self) -> ListIpWhitelistResponse:
        """Retrieve the active IP and CIDR whitelist entries for the current user asynchronously.

        Returns:
            ListIpWhitelistResponse with entries list and count.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/security/ip-whitelist", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return ListIpWhitelistResponse.model_validate(resp.json())

    async def add_ip_whitelist(
        self, ip_or_cidr: str, description: Optional[str] = None
    ) -> IpWhitelistEntry:
        """Add an IP address or CIDR range to the access whitelist asynchronously.

        Args:
            ip_or_cidr: IP address (e.g. '192.168.1.1') or CIDR subnet (e.g. '203.0.113.0/24').
            description: Optional note or description for this entry.

        Returns:
            IpWhitelistEntry representing the created whitelist rule.
        """
        if not ip_or_cidr or not ip_or_cidr.strip():
            raise FinTextValidationError("ip_or_cidr cannot be empty")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        payload = {"ip_or_cidr": ip_or_cidr.strip()}
        if description is not None:
            payload["description"] = description

        try:
            resp = await self._client.post("/security/ip-whitelist", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return IpWhitelistEntry.model_validate(resp.json())

    async def delete_ip_whitelist(self, entry_id: str) -> DeleteIpWhitelistResponse:
        """Remove an IP whitelist entry by ID asynchronously.

        Args:
            entry_id: UUID of the whitelist entry to remove.

        Returns:
            DeleteIpWhitelistResponse with status and ID.
        """
        if not entry_id or not entry_id.strip():
            raise FinTextValidationError("entry_id cannot be empty")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/security/ip-whitelist/{entry_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return DeleteIpWhitelistResponse.model_validate(resp.json())

    # ─── News Articles & Full Text ───────────────────────────────────────────

    async def list_news_articles(
        self,
        ticker: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        source: Optional[str] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> NewsArticlesListResponse:
        """List news articles with metadata and 200-character preview snippets asynchronously.

        Args:
            ticker: Optional ticker filter (e.g. 'AAPL').
            start_date: Optional start date filter (YYYY-MM-DD or RFC3339).
            end_date: Optional end date filter (YYYY-MM-DD or RFC3339).
            source: Optional news source filter.
            limit: Maximum articles per page (1 to 100, default: 20).
            offset: Pagination offset (default: 0).

        Returns:
            NewsArticlesListResponse with list of article previews and total count.
        """
        if limit < 1 or limit > 100:
            raise FinTextValidationError("limit must be between 1 and 100")
        if offset < 0:
            raise FinTextValidationError("offset must be non-negative")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        params: dict[str, Any] = {"limit": limit, "offset": offset}
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()
        if source:
            params["source"] = source.strip()

        try:
            resp = await self._client.get("/news/articles", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return NewsArticlesListResponse.model_validate(resp.json())

    async def get_news_article(self, article_id: str) -> NewsArticleFull:
        """Retrieve full text content and detailed NLP signals for a specific news article asynchronously.

        Args:
            article_id: UUID of the news article.

        Returns:
            NewsArticleFull containing the full body text and metrics.
        """
        if not article_id or not article_id.strip():
            raise FinTextValidationError("article_id cannot be empty")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/news/articles/{article_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return NewsArticleFull.model_validate(resp.json())

    # ─── Entity Sentiment Breakdown ──────────────────────────────────────────

    async def sentiment_entities(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_mentions: int = 5,
        entity_type: str = "all",
        limit: int = 20,
        sort_by: str = "avg_sentiment",
    ) -> EntitySentimentResponse:
        """Retrieve aggregated sentiment statistics across extracted financial named entities asynchronously.

        Args:
            start_date: Optional start date (YYYY-MM-DD or RFC3339). Defaults to last 7 days.
            end_date: Optional end date (YYYY-MM-DD or RFC3339). Defaults to now.
            min_mentions: Minimum article mention count required (default: 5, min: 1).
            entity_type: Filter by entity type ('all', 'company', 'person', 'product', 'location', 'organization').
            limit: Maximum entities to return (1 to 100, default: 20).
            sort_by: Sorting field ('avg_sentiment', 'mentions', 'positive_ratio', 'negative_ratio').

        Returns:
            EntitySentimentResponse containing the aggregated entity metrics.
        """
        if min_mentions < 1:
            raise FinTextValidationError("min_mentions must be at least 1")
        if limit < 1 or limit > 100:
            raise FinTextValidationError("limit must be between 1 and 100")
        
        allowed_types = {"all", "company", "person", "product", "location", "organization"}
        if entity_type.strip().lower() not in allowed_types:
            raise FinTextValidationError(f"Invalid entity_type '{entity_type}'. Allowed: {allowed_types}")

        allowed_sorts = {"avg_sentiment", "mentions", "positive_ratio", "negative_ratio"}
        if sort_by.strip().lower() not in allowed_sorts:
            raise FinTextValidationError(f"Invalid sort_by '{sort_by}'. Allowed: {allowed_sorts}")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        params: dict[str, Any] = {
            "min_mentions": min_mentions,
            "entity_type": entity_type.strip().lower(),
            "limit": limit,
            "sort_by": sort_by.strip().lower(),
        }
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        try:
            resp = await self._client.get("/sentiment/entities", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return EntitySentimentResponse.model_validate(resp.json())

    async def get_audit_logs(
        self,
        *,
        action: Optional[str] = None,
        user_id: Optional[str] = None,
        org_id: Optional[str] = None,
        entity_type: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        limit: int = 100,
        offset: int = 0,
    ) -> AuditLogsResponse:
        """
        Query compliance audit logs asynchronously with filtering and pagination.

        Parameters:
            action: Optional filter by audit action code (e.g., 'apikey.create', 'org.created').
            user_id: Optional filter by user ID (restricted to self unless org admin/global admin).
            org_id: Optional filter by organization UUID.
            entity_type: Optional filter by entity type (e.g., 'api_key', 'organization').
            start_date: Optional start datetime filter (ISO 8601 or YYYY-MM-DD).
            end_date: Optional end datetime filter (ISO 8601 or YYYY-MM-DD).
            limit: Maximum records to return per page (1..=1000, default: 100).
            offset: Number of records to skip (default: 0).

        Returns:
            AuditLogsResponse containing matching audit log records and pagination metadata.
        """
        if limit < 1 or limit > 1000:
            raise FinTextValidationError("limit must be between 1 and 1000")
        if offset < 0:
            raise FinTextValidationError("offset must be non-negative")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        params: dict[str, Any] = {
            "limit": limit,
            "offset": offset,
        }
        if action:
            params["action"] = action.strip()
        if user_id:
            params["user_id"] = user_id.strip()
        if org_id:
            params["org_id"] = org_id.strip()
        if entity_type:
            params["entity_type"] = entity_type.strip()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        try:
            resp = await self._client.get("/audit/logs", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return AuditLogsResponse.model_validate(resp.json())

    async def export_audit_logs(
        self,
        *,
        format: str = "json",
        action: Optional[str] = None,
        user_id: Optional[str] = None,
        org_id: Optional[str] = None,
        entity_type: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> Union[str, AuditLogExportResponse]:
        """
        Export compliance audit logs asynchronously formatted as CSV or JSON.

        Parameters:
            format: Target format ('csv' or 'json', default: 'json').
            action: Optional filter by audit action code.
            user_id: Optional filter by user ID.
            org_id: Optional filter by organization UUID.
            entity_type: Optional filter by entity type.
            start_date: Optional start datetime filter.
            end_date: Optional end datetime filter.

        Returns:
            CSV string if format is 'csv', or AuditLogExportResponse if format is 'json'.
        """
        fmt = format.strip().lower()
        if fmt not in {"csv", "json"}:
            raise FinTextValidationError(f"Invalid format '{format}'. Allowed formats: 'csv', 'json'.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        params: dict[str, Any] = {"format": fmt}
        if action:
            params["action"] = action.strip()
        if user_id:
            params["user_id"] = user_id.strip()
        if org_id:
            params["org_id"] = org_id.strip()
        if entity_type:
            params["entity_type"] = entity_type.strip()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        try:
            resp = await self._client.get("/audit/export", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        if fmt == "csv":
            return resp.text
        return AuditLogExportResponse.model_validate(resp.json())

    async def create_api_key(self, name: Optional[str] = None) -> CreateApiKeyResponse:
        """
        Generate a new long-lived API key token asynchronously.

        Parameters:
            name: Optional friendly name for the key.

        Returns:
            CreateApiKeyResponse containing key metadata and the full plaintext API key.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        body: dict[str, Any] = {}
        if name:
            body["name"] = name.strip()

        try:
            resp = await self._client.post("/auth/api-keys", json=body, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return CreateApiKeyResponse.model_validate(resp.json())

    async def list_api_keys(self) -> ListApiKeysResponse:
        """
        List all active and rotating API keys owned by the authenticated user asynchronously.

        Returns:
            ListApiKeysResponse containing the list of API key items and count.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/auth/api-keys", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return ListApiKeysResponse.model_validate(resp.json())

    async def get_api_key(self, api_key_id: str) -> ApiKeyItem:
        """
        Retrieve metadata and rotation status for a specific API key asynchronously.

        Parameters:
            api_key_id: UUID of the API key.

        Returns:
            ApiKeyItem containing key details.
        """
        if not api_key_id or not api_key_id.strip():
            raise FinTextValidationError("api_key_id cannot be empty.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/auth/api-keys/{api_key_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return ApiKeyItem.model_validate(resp.json())

    async def rotate_api_key(
        self,
        api_key_id: str,
        overlap_hours: int = 24,
    ) -> RotateApiKeyResponse:
        """
        Initiate zero-downtime rotation for an API key asynchronously.

        Parameters:
            api_key_id: UUID of the API key to rotate.
            overlap_hours: Number of hours (1 to 168, default: 24) the old key remains valid.

        Returns:
            RotateApiKeyResponse with new key material and old key expiration time.
        """
        if not api_key_id or not api_key_id.strip():
            raise FinTextValidationError("api_key_id cannot be empty.")
        if overlap_hours < 1 or overlap_hours > 168:
            raise FinTextValidationError(f"overlap_hours must be between 1 and 168, got {overlap_hours}.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        body = {"overlap_hours": overlap_hours}

        try:
            resp = await self._client.post(
                f"/auth/api-keys/{api_key_id.strip()}/rotate",
                json=body,
                headers=headers,
            )
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return RotateApiKeyResponse.model_validate(resp.json())

    async def revoke_api_key(self, api_key_id: str) -> dict[str, Any]:
        """
        Revoke an existing API key immediately asynchronously.

        Parameters:
            api_key_id: UUID of the API key to revoke.

        Returns:
            Dict containing status and message.
        """
        if not api_key_id or not api_key_id.strip():
            raise FinTextValidationError("api_key_id cannot be empty.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/auth/api-keys/{api_key_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return resp.json()

    async def search(
        self,
        q: str,
        types: Union[str, list[str]] = "all",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> SearchResponse:
        """
        Execute unified cross-domain search across news, transcripts, filings, sentiment, events, insider trades, supply chain, and options.

        Parameters:
            q: Search query term, keyword, or ticker symbol (required).
            types: Data domains to include ('all' or list/comma-separated string).
            start_date: Optional start date filter (YYYY-MM-DD or RFC3339).
            end_date: Optional end date filter (YYYY-MM-DD or RFC3339).
            limit: Maximum total results to return (1 to 100, default: 20).
            offset: Pagination offset (default: 0).

        Returns:
            SearchResponse containing ranked results with match relevance scores.
        """
        if not q or not q.strip():
            raise FinTextValidationError("Search query 'q' cannot be empty.")
        if limit < 1 or limit > 100:
            raise FinTextValidationError("limit must be between 1 and 100.")
        if offset < 0:
            raise FinTextValidationError("offset must be non-negative.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        types_str = ",".join(types) if isinstance(types, list) else types

        params: dict[str, Any] = {
            "q": q.strip(),
            "types": types_str,
            "limit": limit,
            "offset": offset,
        }
        if start_date:
            params["start_date"] = start_date
        if end_date:
            params["end_date"] = end_date

        try:
            resp = await self._client.get("/search", headers=headers, params=params)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return SearchResponse.model_validate(resp.json())

    async def sector_rotation(
        self,
        lookback_days: int = 30,
        include_momentum: bool = True,
        top_n: int = 3,
        min_confidence: float = 0.0,
    ) -> SectorRotationResponse:
        """
        Query sector rotation signals and relative strength rankings across GICS sectors.

        Args:
            lookback_days: Number of lookback days for scoring (default: 30, min: 1, max: 90).
            include_momentum: Include price momentum in composite score (default: True).
            top_n: Number of top/bottom sectors to flag (default: 3, min: 1, max: 10).
            min_confidence: Minimum sentiment confidence threshold 0.0–1.0 (default: 0.0).

        Returns:
            SectorRotationResponse with ranked sector items, outperform/underperform lists, and market signal.
        """
        if lookback_days < 1 or lookback_days > 90:
            raise FinTextValidationError("Field 'lookback_days' must be between 1 and 90.")
        if top_n < 1 or top_n > 10:
            raise FinTextValidationError("Field 'top_n' must be between 1 and 10.")
        if min_confidence < 0.0 or min_confidence > 1.0:
            raise FinTextValidationError("Field 'min_confidence' must be between 0.0 and 1.0.")

        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "lookback_days": lookback_days,
            "include_momentum": str(include_momentum).lower(),
            "top_n": top_n,
            "min_confidence": min_confidence,
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/market/sector-rotation", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SectorRotationResponse.model_validate(resp.json())

    async def get_digest_subscription(self) -> DigestSubscriptionResponse:
        """
        Retrieve the authenticated user's current email digest subscription configuration.

        Returns:
            DigestSubscriptionResponse containing active subscription details.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/digest/subscription", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DigestSubscriptionResponse.model_validate(resp.json())

    async def create_digest_subscription(
        self,
        frequency: str = "daily",
        tickers: Optional[list[str]] = None,
        sectors: Optional[list[str]] = None,
        event_types: Optional[list[str]] = None,
        is_active: bool = True,
    ) -> DigestSubscriptionResponse:
        """
        Create or update the email digest subscription for the authenticated user.

        Args:
            frequency: Delivery cadence ('daily' or 'weekly'). Default: 'daily'.
            tickers: Optional list of ticker symbols to track.
            sectors: Optional list of GICS sector names to track.
            event_types: Optional list of catalyst event categories ('earnings', 'insider', 'ma', '8k', 'news', 'sentiment').
            is_active: Whether subscription is active (default: True).

        Returns:
            DigestSubscriptionResponse with updated configuration.
        """
        await self._ensure_authenticated()

        payload: dict[str, Any] = {
            "frequency": frequency.strip().lower(),
            "tickers": [t.strip().upper() for t in tickers] if tickers else [],
            "sectors": [s.strip() for s in sectors] if sectors else [],
            "event_types": [e.strip().lower() for e in event_types] if event_types else ["earnings", "insider", "8k", "news"],
            "is_active": is_active,
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/digest/subscription", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DigestSubscriptionResponse.model_validate(resp.json())

    async def delete_digest_subscription(self) -> DeleteDigestResponse:
        """
        Delete/deactivate the authenticated user's email digest subscription.

        Returns:
            DeleteDigestResponse confirming deletion.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete("/digest/subscription", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeleteDigestResponse.model_validate(resp.json())

    async def trigger_digest(
        self,
        recipient_email: Optional[str] = None,
        format: Optional[str] = "html",
    ) -> TriggerDigestResponse:
        """
        Trigger on-demand compilation and delivery of an email market digest.

        Args:
            recipient_email: Optional override recipient email address.
            format: Output format preference ('html' or 'text'). Default: 'html'.

        Returns:
            TriggerDigestResponse containing digest preview, recipient, subject, and item breakdown.
        """
        await self._ensure_authenticated()

        payload: dict[str, Any] = {}
        if recipient_email:
            payload["recipient_email"] = recipient_email.strip()
        if format:
            payload["format"] = format.strip().lower()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/digest/trigger", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return TriggerDigestResponse.model_validate(resp.json())

    # ── Streaming Kafka Topic Access ─────────────────────────────────────────

    async def list_kafka_topics(self) -> KafkaTopicsResponse:
        """
        List all available institutional Kafka streaming topics with schemas and metadata.

        Returns:
            KafkaTopicsResponse containing array of topic descriptions and schemas.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/stream/kafka/topics", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return KafkaTopicsResponse.model_validate(resp.json())

    async def get_kafka_credentials(
        self,
        topic: str,
        ttl_minutes: Optional[int] = 60,
        consumer_group: Optional[str] = None,
    ) -> KafkaCredentials:
        """
        Request temporary Kafka consumer credentials for a specific streaming topic.

        Args:
            topic: Target topic to stream (e.g. 'sentiment-events', 'news-events', 'options-events').
            ttl_minutes: Credential validity duration in minutes (default 60, min 1, max 1440).
            consumer_group: Optional custom consumer group name.

        Returns:
            KafkaCredentials containing username, plaintext password, broker address, and expiration.
        """
        await self._ensure_authenticated()

        if not topic or not topic.strip():
            raise FinTextValidationError("Field 'topic' cannot be empty.")

        params: dict[str, Any] = {"topic": topic.strip().lower()}
        if ttl_minutes is not None:
            if ttl_minutes < 1 or ttl_minutes > 1440:
                raise FinTextValidationError("Field 'ttl_minutes' must be between 1 and 1440.")
            params["ttl_minutes"] = ttl_minutes
        if consumer_group:
            params["consumer_group"] = consumer_group.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/stream/kafka/credentials", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return KafkaCredentials.model_validate(resp.json())

    async def revoke_kafka_credentials(
        self,
        credential_id: Union[str, Any],
    ) -> RevokeKafkaCredentialsResponse:
        """
        Revoke Kafka consumer credentials early before their scheduled expiration.

        Args:
            credential_id: Unique UUID string of credentials to revoke.

        Returns:
            RevokeKafkaCredentialsResponse confirmation.
        """
        await self._ensure_authenticated()

        cid_str = str(credential_id).strip()
        if not cid_str:
            raise FinTextValidationError("Field 'credential_id' cannot be empty.")

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/stream/kafka/credentials/{cid_str}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RevokeKafkaCredentialsResponse.model_validate(resp.json())

    async def get_retention_policies(self) -> RetentionPoliciesResponse:
        """
        List active data retention policies for the authenticated user and organization.

        Returns:
            RetentionPoliciesResponse containing list of configured retention policies.
        """
        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/retention/policies", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RetentionPoliciesResponse.model_validate(resp.json())

    async def create_retention_policy(
        self,
        data_category: str,
        retention_days: int,
        is_active: bool = True,
    ) -> RetentionPolicy:
        """
        Create or update a compliance data retention policy for a specific category.

        Args:
            data_category: Target data category ('usage_events', 'audit_logs', etc.)
            retention_days: Retention duration in days (1 to 3650)
            is_active: Whether policy is actively enforced (default: True)

        Returns:
            RetentionPolicy configured entity.
        """
        await self._ensure_authenticated()

        if not data_category or not data_category.strip():
            raise FinTextValidationError("Field 'data_category' cannot be empty.")
        if retention_days < 1 or retention_days > 3650:
            raise FinTextValidationError(f"Field 'retention_days' must be between 1 and 3650 (got {retention_days}).")

        headers = {
            "Authorization": f"Bearer {self.api_token}",
            "Content-Type": "application/json",
        }
        body = {
            "data_category": data_category.strip(),
            "retention_days": retention_days,
            "is_active": is_active,
        }

        try:
            resp = await self._client.post("/retention/policies", headers=headers, json=body)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RetentionPolicy.model_validate(resp.json())

    async def delete_retention_policy(
        self,
        policy_id: Union[str, Any],
    ) -> DeleteRetentionPolicyResponse:
        """
        Delete or deactivate a data retention policy by ID.

        Args:
            policy_id: Unique UUID string of policy to delete.

        Returns:
            DeleteRetentionPolicyResponse confirmation.
        """
        await self._ensure_authenticated()

        pid_str = str(policy_id).strip()
        if not pid_str:
            raise FinTextValidationError("Field 'policy_id' cannot be empty.")

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/retention/policies/{pid_str}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeleteRetentionPolicyResponse.model_validate(resp.json())

    async def factor_exposure(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        factors: Optional[Union[str, List[str]]] = None,
        benchmark_ticker: Optional[str] = None,
    ) -> FactorExposureResponse:
        """
        Compute multi-factor risk exposure regression report for a stock asynchronously.

        Args:
            ticker: Underlying equity ticker symbol (e.g. 'AAPL').
            start_date: Historical estimation window start date (YYYY-MM-DD).
            end_date: Historical estimation window end date (YYYY-MM-DD).
            factors: Optional list or comma-separated string of risk factors (default: 'market,momentum,sentiment,volatility').
                     Allowed: 'market', 'momentum', 'sentiment', 'volatility', 'size', 'value'.
            benchmark_ticker: Optional benchmark ticker for market factor (default: 'SPY').

        Returns:
            FactorExposureResponse with factor betas, t-stats, p-values, and OLS model metrics.
        """
        await self._ensure_authenticated()

        t_clean = ticker.strip().upper()
        if not t_clean:
            raise FinTextValidationError("Field 'ticker' cannot be empty.")

        params: dict[str, Any] = {
            "ticker": t_clean,
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
        }

        if factors is not None:
            if isinstance(factors, list):
                params["factors"] = ",".join(f.strip() for f in factors if f.strip())
            else:
                params["factors"] = str(factors).strip()

        if benchmark_ticker is not None:
            params["benchmark_ticker"] = benchmark_ticker.strip().upper()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/risk/factor-exposure", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return FactorExposureResponse.model_validate(resp.json())

    async def esg_scores(
        self,
        ticker: Optional[str] = None,
        sector: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_confidence: float = 0.0,
    ) -> ESGScoresResponse:
        """
        Retrieve quantitative Environmental, Social, and Governance (ESG) sentiment scores,
        keyword mentions, positive/negative ratios, and composite ESG ratings asynchronously.

        Args:
            ticker: Optional equity ticker symbol (e.g. 'AAPL').
            sector: Optional GICS sector name (e.g. 'Technology', 'Energy').
            start_date: Optional news analysis start date (YYYY-MM-DD, default: 90 days ago).
            end_date: Optional news analysis end date (YYYY-MM-DD, default: today).
            min_confidence: Minimum sentiment confidence threshold (0.0 to 1.0, default: 0.0).

        Returns:
            ESGScoresResponse with dimensional breakdowns and overall score (0 to 100).
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "min_confidence": min_confidence,
        }
        if ticker:
            params["ticker"] = ticker.strip().upper()
        if sector:
            params["sector"] = sector.strip()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/esg/scores", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ESGScoresResponse.model_validate(resp.json())

    async def bankruptcy_risk(
        self,
        ticker: str,
        lookback_days: int = 30,
        include_components: bool = True,
    ) -> BankruptcyRiskResponse:
        """
        Compute quantitative bankruptcy risk score (0-100) and risk category for a stock ticker asynchronously,
        synthesizing SEC 8-K distress events, sentiment deterioration, options PCR/IV,
        supply chain risk propagation, and insider selling pressure.

        Args:
            ticker: Target equity ticker symbol (e.g. 'AAPL', 'BBBY').
            lookback_days: Lookback observation window in calendar days (default: 30, max: 90).
            include_components: Whether to return underlying 6-pillar distress scores (default: True).

        Returns:
            BankruptcyRiskResponse with composite score, category, and optional component breakdown.
        """
        await self._ensure_authenticated()

        t_clean = ticker.strip().upper()
        if not t_clean:
            raise FinTextValidationError("Field 'ticker' cannot be empty.")

        params: dict[str, Any] = {
            "ticker": t_clean,
            "lookback_days": lookback_days,
            "include_components": include_components,
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/risk/bankruptcy", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return BankruptcyRiskResponse.model_validate(resp.json())

    async def fx_sentiment(
        self,
        currency_pair: str = "EUR/USD",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_confidence: float = 0.0,
        limit: int = 20,
    ) -> FXSentimentResponse:
        """
        Retrieve real-time and historical news sentiment scores, central bank policy tone,
        mention counts, and key driver articles across G10 major currency pairs asynchronously.

        Args:
            currency_pair: Target major currency pair (e.g. 'EUR/USD', 'USD/JPY', 'GBP/USD').
            start_date: Start date for news analysis window (YYYY-MM-DD, optional).
            end_date: End date for news analysis window (YYYY-MM-DD, optional).
            min_confidence: Minimum model confidence threshold (0.0 to 1.0, default: 0.0).
            limit: Maximum number of top driving news articles to return (1 to 100, default: 20).

        Returns:
            FXSentimentResponse with summary statistics and top relevant news articles.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "currency_pair": currency_pair.strip(),
            "min_confidence": min_confidence,
            "limit": limit,
        }
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/fx/sentiment", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return FXSentimentResponse.model_validate(resp.json())

    async def commodity_sentiment(
        self,
        commodity: str = "crude_oil",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_confidence: float = 0.0,
        limit: int = 10,
    ) -> CommoditySentimentResponse:
        """
        Retrieve real-time and historical news sentiment scores, supply/demand balance tone,
        mention counts, and key driving articles across major commodity assets asynchronously.

        Args:
            commodity: Target commodity asset (e.g. 'crude_oil', 'gold', 'copper', 'natural_gas', 'wheat', 'silver').
            start_date: Start date for news analysis window (YYYY-MM-DD, optional).
            end_date: End date for news analysis window (YYYY-MM-DD, optional).
            min_confidence: Minimum model confidence threshold (0.0 to 1.0, default: 0.0).
            limit: Maximum number of top driving news articles to return (1 to 50, default: 10).

        Returns:
            CommoditySentimentResponse with summary statistics and top relevant news articles.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "commodity": commodity.strip(),
            "min_confidence": min_confidence,
            "limit": limit,
        }
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/commodities/sentiment", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return CommoditySentimentResponse.model_validate(resp.json())

    async def crypto_sentiment(
        self,
        asset: Optional[str] = "BTC",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        min_confidence: Optional[float] = None,
        limit: Optional[int] = 10,
    ) -> CryptoSentimentResponse:
        """
        Retrieve quantitative news sentiment analytics for major cryptocurrencies asynchronously.

        Args:
            asset: Target cryptocurrency symbol (e.g. 'BTC', 'ETH', 'SOL', 'BNB', 'XRP', 'ADA').
            start_date: Start date for news analysis window (YYYY-MM-DD). Defaults to 30 days ago.
            end_date: End date for news analysis window (YYYY-MM-DD). Defaults to current date.
            min_confidence: Minimum model confidence threshold (0.0 to 1.0).
            limit: Maximum number of driving news articles to return (1 to 50, default: 10).

        Returns:
            CryptoSentimentResponse with summary statistics and top driving news articles.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {}
        if asset:
            params["asset"] = asset.strip()
        if min_confidence is not None:
            params["min_confidence"] = min_confidence
        if limit is not None:
            params["limit"] = limit
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/crypto/sentiment", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return CryptoSentimentResponse.model_validate(resp.json())

    async def options_microstructure(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        metric: str = "both",
        interval: str = "daily",
        limit: int = 100,
    ) -> MicrostructureResponse:
        """
        Retrieve historical time series of Volume-Synchronized Probability of Informed Trading (VPIN)
        and Dealer Gamma Exposure (GEX) for an underlying ticker asynchronously.

        Args:
            ticker: Target underlying equity ticker symbol (e.g. 'AAPL', 'NVDA', 'SPY').
            start_date: Start date for microstructure analysis window (YYYY-MM-DD).
            end_date: End date for microstructure analysis window (YYYY-MM-DD).
            metric: Metric to retrieve ('vpin', 'gex', or 'both', default: 'both').
            interval: Aggregation interval ('daily' or 'intraday', default: 'daily').
            limit: Maximum number of observation points to return (1 to 1000, default: 100).

        Returns:
            MicrostructureResponse with time series data points.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "metric": metric.strip().lower(),
            "interval": interval.strip().lower(),
            "limit": limit,
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/options/microstructure", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return MicrostructureResponse.model_validate(resp.json())

    async def market_breadth(
        self,
        start_date: str,
        end_date: str,
        universe: str = "all",
        limit: int = 50,
        include_new_highs_lows: bool = True,
    ) -> MarketBreadthResponse:
        """
        Retrieve daily market breadth, advance/decline volume & counts, advance/decline
        ratio, breadth index, and 52-week new highs/lows for a specified universe asynchronously.

        Args:
            start_date: Start date for breadth analysis window (YYYY-MM-DD).
            end_date: End date for breadth analysis window (YYYY-MM-DD).
            universe: Constituent universe ('all', 'sp500', or comma-separated list of tickers).
            limit: Maximum number of observation days to return (1 to 200, default: 50).
            include_new_highs_lows: Whether to calculate and include 52-week highs and lows (default: True).

        Returns:
            MarketBreadthResponse with daily market breadth time series data points.
        """
        await self._ensure_authenticated()

        params: dict[str, Any] = {
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "universe": universe.strip(),
            "limit": limit,
            "include_new_highs_lows": include_new_highs_lows,
        }

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/market/breadth", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return MarketBreadthResponse.model_validate(resp.json())

    async def create_polling_webhook(
        self,
        name: str,
        url: str,
        interval_seconds: int = 300,
        query_type: str = "sentiment",
        query_params: Optional[dict[str, Any]] = None,
    ) -> PollingWebhook:
        """
        Create a new custom polling webhook subscription asynchronously.

        Args:
            name: User-friendly name for this polling job.
            url: Destination webhook receiver URL (HTTPS or localhost HTTP).
            interval_seconds: Polling frequency in seconds (60 to 86400).
            query_type: Category to query ('sentiment', 'news', 'events', 'options').
            query_params: JSON query filters (e.g. {"tickers": ["AAPL", "MSFT"]}).

        Returns:
            Created PollingWebhook object including HMAC-SHA256 secret.
        """
        await self._ensure_authenticated()

        body = {
            "name": name.strip(),
            "url": url.strip(),
            "interval_seconds": interval_seconds,
            "query_type": query_type.strip(),
            "query_params": query_params or {},
        }
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/polling-webhooks", json=body, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PollingWebhook.model_validate(resp.json())

    async def list_polling_webhooks(self) -> PollingWebhooksResponse:
        """
        List all custom polling webhook subscriptions for the authenticated user asynchronously.

        Returns:
            PollingWebhooksResponse containing list of polling webhooks.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/polling-webhooks", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PollingWebhooksResponse.model_validate(resp.json())

    async def delete_polling_webhook(self, webhook_id: str) -> DeletePollingWebhookResponse:
        """
        Delete a custom polling webhook subscription asynchronously.

        Args:
            webhook_id: UUID of the polling webhook to delete.

        Returns:
            DeletePollingWebhookResponse confirmation.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/polling-webhooks/{webhook_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeletePollingWebhookResponse.model_validate(resp.json())

    async def create_chat_alert(
        self,
        channel_type: str,
        channel_target: str,
        event_types: list[str],
    ) -> ChatAlertSubscription:
        """
        Create a new Telegram or Discord chat alert bot subscription asynchronously.

        Args:
            channel_type: 'telegram' or 'discord'.
            channel_target: Telegram chat ID (or @channel) or Discord HTTPS webhook URL.
            event_types: List of market event types (e.g. ['sentiment_anomaly', '8k_filing', 'unusual_options']).

        Returns:
            ChatAlertSubscription with unique ID and subscription details.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        payload = {
            "channel_type": channel_type.strip(),
            "channel_target": channel_target.strip(),
            "event_types": [et.strip() for et in event_types],
        }

        try:
            resp = await self._client.post("/chat-alerts", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ChatAlertSubscription.model_validate(resp.json())

    async def list_chat_alerts(self) -> ChatAlertsResponse:
        """
        List all active chat alert bot subscriptions configured for the authenticated user asynchronously.

        Returns:
            ChatAlertsResponse with list of subscriptions.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/chat-alerts", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ChatAlertsResponse.model_validate(resp.json())

    async def delete_chat_alert(self, subscription_id: str) -> DeleteChatAlertResponse:
        """
        Delete a chat alert subscription asynchronously.

        Args:
            subscription_id: UUID of the chat alert subscription to delete.

        Returns:
            DeleteChatAlertResponse confirmation.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/chat-alerts/{subscription_id.strip()}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DeleteChatAlertResponse.model_validate(resp.json())

    async def credit_sentiment(
        self,
        ticker: str,
        lookback_days: int = 30,
    ) -> CreditSentimentResponse:
        """
        Retrieve corporate Credit Default Sentiment and multi-signal credit distress metrics asynchronously.

        Args:
            ticker: Target stock ticker symbol (e.g. 'AAPL', 'MSFT').
            lookback_days: Lookback observation window in calendar days (1 to 90, default 30).

        Returns:
            CreditSentimentResponse with composite score, news sentiment, 8-K distress counts, PCR, and IV.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}
        params = {
            "ticker": ticker.strip().upper(),
            "lookback_days": lookback_days,
        }

        try:
            resp = await self._client.get("/risk/credit-sentiment", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return CreditSentimentResponse.model_validate(resp.json())

    async def backfill_sentiment(
        self,
        ticker: str,
        start_date: str,
        end_date: str,
        limit: int = 1000,
        overwrite: bool = False,
    ) -> BackfillSentimentResponse:
        """
        Request historical news sentiment backfill asynchronously for a given ticker and date window.

        Args:
            ticker: Target stock ticker symbol (e.g. 'AAPL').
            start_date: Historical start date (YYYY-MM-DD).
            end_date: Historical end date (YYYY-MM-DD).
            limit: Maximum number of articles to process (default 1000, max 10000).
            overwrite: Whether to recompute sentiment for articles with existing scores (default False).

        Returns:
            BackfillSentimentResponse with summary of processed and failed articles.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}", "Content-Type": "application/json"}
        payload = {
            "ticker": ticker.strip().upper(),
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "limit": limit,
            "overwrite": overwrite,
        }

        try:
            resp = await self._client.post("/sentiment/backfill", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return BackfillSentimentResponse.model_validate(resp.json())

    async def portfolio_optimize(
        self,
        tickers: List[str],
        start_date: str,
        end_date: str,
        optimization_type: str = "max_sharpe",
        risk_free_rate: float = 0.0,
        long_only: bool = True,
    ) -> PortfolioOptimizeResponse:
        """
        Compute optimal portfolio weights asynchronously using Mean-Variance (Max Sharpe) or Risk Parity.

        Args:
            tickers: List of constituent stock tickers (2 to 20 assets).
            start_date: Historical analysis window start date (YYYY-MM-DD).
            end_date: Historical analysis window end date (YYYY-MM-DD).
            optimization_type: Optimization method ('max_sharpe' or 'risk_parity', default 'max_sharpe').
            risk_free_rate: Annualized risk-free rate (0.0 to 0.10, default 0.0).
            long_only: Whether to enforce long-only weights (default True).

        Returns:
            PortfolioOptimizeResponse with optimal asset weights and expected risk/return metrics.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}", "Content-Type": "application/json"}
        payload = {
            "tickers": [t.strip().upper() for t in tickers],
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "optimization_type": optimization_type.strip().lower(),
            "risk_free_rate": risk_free_rate,
            "constraints": {
                "long_only": long_only,
            },
        }

        try:
            resp = await self._client.post("/portfolio/optimize", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PortfolioOptimizeResponse.model_validate(resp.json())

    async def portfolio_factor_exposure(
        self,
        tickers: List[str],
        weights: List[float],
        start_date: str,
        end_date: str,
        benchmark_ticker: Optional[str] = None,
        factors: Optional[Union[str, List[str]]] = None,
    ) -> PortfolioFactorExposureResponse:
        """
        Compute multi-factor risk exposure and attribution regression report for a portfolio asynchronously.

        Args:
            tickers: Portfolio constituent stock ticker symbols (2 to 20 assets).
            weights: Allocation weights corresponding to tickers (must sum to 1.0 ± 0.05).
            start_date: Historical estimation window start date (YYYY-MM-DD).
            end_date: Historical estimation window end date (YYYY-MM-DD).
            benchmark_ticker: Optional benchmark ticker for market factor (default: 'SPY').
            factors: Optional list or comma-separated string of risk factors (default: 'market,momentum,sentiment,volatility').
                     Allowed: 'market', 'momentum', 'sentiment', 'volatility', 'size', 'value'.

        Returns:
            PortfolioFactorExposureResponse with portfolio factor betas, t-stats, p-values, and OLS model metrics.
        """
        await self._ensure_authenticated()

        if not tickers or len(tickers) < 2:
            raise FinTextValidationError("Portfolio must contain at least 2 tickers.")
        if len(tickers) != len(weights):
            raise FinTextValidationError(f"Length of tickers ({len(tickers)}) must match weights ({len(weights)}).")

        payload: Dict[str, Any] = {
            "tickers": [t.strip().upper() for t in tickers],
            "weights": [float(w) for w in weights],
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
        }

        if benchmark_ticker is not None:
            payload["benchmark_ticker"] = benchmark_ticker.strip().upper()

        if factors is not None:
            if isinstance(factors, list):
                payload["factors"] = ",".join(f.strip() for f in factors if f.strip())
            else:
                payload["factors"] = str(factors).strip()

        headers = {"Authorization": f"Bearer {self.api_token}", "Content-Type": "application/json"}

        try:
            resp = await self._client.post("/risk/portfolio-factor-exposure", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PortfolioFactorExposureResponse.model_validate(resp.json())

    async def create_retraining_job(
        self,
        model_type: Optional[str] = "sentiment",
        trigger_type: Optional[str] = "manual",
        config: Optional[Dict[str, Any]] = None,
        request: Optional[Union[CreateRetrainingJobRequest, Dict[str, Any]]] = None,
    ) -> RetrainingJobResponse:
        """Create and trigger a new model retraining job asynchronously.

        Args:
            model_type: Target model type (e.g. 'sentiment', 'finbert', 'minilm', default: 'sentiment').
            trigger_type: Trigger mechanism ('manual' or 'scheduled', default: 'manual').
            config: Optional hyperparameter and dataset configuration dictionary.
            request: Optional CreateRetrainingJobRequest instance or dictionary.

        Returns:
            RetrainingJobResponse containing the created job details.
        """
        await self._ensure_authenticated()

        if request is not None:
            if isinstance(request, dict):
                req_model = CreateRetrainingJobRequest.model_validate(request)
            elif isinstance(request, CreateRetrainingJobRequest):
                req_model = request
            else:
                raise FinTextValidationError("Request must be a CreateRetrainingJobRequest or dict.")
        else:
            req_model = CreateRetrainingJobRequest(
                model_type=model_type,
                trigger_type=trigger_type,
                config=config,
            )

        payload = req_model.model_dump(exclude_none=True)
        headers = {"Authorization": f"Bearer {self.api_token}", "Content-Type": "application/json"}

        try:
            resp = await self._client.post("/retraining/jobs", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RetrainingJobResponse.model_validate(resp.json())

    async def list_retraining_jobs(
        self,
        status: Optional[str] = None,
        model_type: Optional[str] = None,
        trigger_type: Optional[str] = None,
        limit: Optional[int] = 50,
        offset: Optional[int] = 0,
    ) -> ListRetrainingJobsResponse:
        """List model retraining jobs with optional filtering and pagination asynchronously.

        Args:
            status: Filter by job status ('pending', 'running', 'completed', 'failed', 'cancelled').
            model_type: Filter by target model type.
            trigger_type: Filter by trigger mechanism.
            limit: Maximum number of jobs to return (default: 50).
            offset: Number of jobs to skip for pagination (default: 0).

        Returns:
            ListRetrainingJobsResponse containing the list of jobs and pagination totals.
        """
        await self._ensure_authenticated()

        params: Dict[str, Any] = {}
        if status is not None:
            params["status"] = status
        if model_type is not None:
            params["model_type"] = model_type
        if trigger_type is not None:
            params["trigger_type"] = trigger_type
        if limit is not None:
            params["limit"] = limit
        if offset is not None:
            params["offset"] = offset

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/retraining/jobs", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ListRetrainingJobsResponse.model_validate(resp.json())

    async def get_retraining_job(
        self,
        job_id: str,
    ) -> RetrainingJobResponse:
        """Retrieve details and status for a specific model retraining job asynchronously.

        Args:
            job_id: Unique UUID string of the retraining job.

        Returns:
            RetrainingJobResponse containing the current job status and metadata.
        """
        await self._ensure_authenticated()

        clean_id = job_id.strip()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/retraining/jobs/{clean_id}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RetrainingJobResponse.model_validate(resp.json())

    async def cancel_retraining_job(
        self,
        job_id: str,
    ) -> RetrainingJobResponse:
        """Cancel a pending or running model retraining job asynchronously.

        Args:
            job_id: Unique UUID string of the retraining job to cancel.

        Returns:
            RetrainingJobResponse containing updated job details.
        """
        await self._ensure_authenticated()

        clean_id = job_id.strip()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post(f"/retraining/jobs/{clean_id}/cancel", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return RetrainingJobResponse.model_validate(resp.json())

    # ── FIX Protocol Bridge & Execution ──────────────────────────────────────

    async def submit_fix_order(
        self,
        fix_message: Union[str, FIXOrderRequest],
    ) -> FIXOrderResponse:
        """Submit a FIX 4.4 order message (NewOrderSingle, 35=D) for simulated broker execution.

        Args:
            fix_message: Raw pipe-delimited FIX 4.4 string or FIXOrderRequest payload.

        Returns:
            FIXOrderResponse containing execution report details and raw FIX response message.
        """
        await self._ensure_authenticated()

        raw_msg = fix_message.fix_message if isinstance(fix_message, FIXOrderRequest) else str(fix_message)
        if not raw_msg or not raw_msg.strip():
            raise FinTextValidationError("FIX order message cannot be empty.")

        headers = {
            "Authorization": f"Bearer {self.api_token}",
            "Content-Type": "application/json",
        }
        body = {"fix_message": raw_msg.strip()}

        try:
            resp = await self._client.post("/fix/order", json=body, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return FIXOrderResponse.model_validate(resp.json())

    async def list_fix_orders(
        self,
        status: Optional[str] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> FIXOrdersListResponse:
        """List FIX orders submitted by the authenticated user with optional status filter and pagination.

        Args:
            status: Optional lifecycle status filter ('open', 'filled', 'cancelled', 'rejected').
            limit: Maximum number of orders to return (default: 20, max: 1000).
            offset: Pagination offset index (default: 0).

        Returns:
            FIXOrdersListResponse containing order summaries and total count.
        """
        await self._ensure_authenticated()

        if limit < 1 or limit > 1000:
            raise FinTextValidationError(f"Limit must be between 1 and 1000, got {limit}.")
        if offset < 0:
            raise FinTextValidationError(f"Offset must be non-negative, got {offset}.")

        params: Dict[str, Any] = {
            "limit": limit,
            "offset": offset,
        }
        if status is not None and status.strip():
            params["status"] = status.strip().lower()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/fix/orders", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return FIXOrdersListResponse.model_validate(resp.json())

    async def cancel_fix_order(
        self,
        fix_cancel_message: Union[str, FIXCancelRequest],
    ) -> FIXOrderResponse:
        """Submit a FIX OrderCancelRequest (35=F) to cancel an active open order.

        Args:
            fix_cancel_message: Raw pipe-delimited FIX cancel string or FIXCancelRequest payload.

        Returns:
            FIXOrderResponse containing the cancellation Execution Report (150=4/39=4).
        """
        await self._ensure_authenticated()

        raw_msg = fix_cancel_message.fix_message if isinstance(fix_cancel_message, FIXCancelRequest) else str(fix_cancel_message)
        if not raw_msg or not raw_msg.strip():
            raise FinTextValidationError("FIX cancel message cannot be empty.")

        headers = {
            "Authorization": f"Bearer {self.api_token}",
            "Content-Type": "application/json",
        }
        body = {"fix_message": raw_msg.strip()}

        try:
            resp = await self._client.post("/fix/cancel", json=body, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return FIXOrderResponse.model_validate(resp.json())

    async def list_dlq_events(
        self,
        source: Optional[str] = None,
        error_type: Optional[str] = None,
        status: Optional[str] = "failed",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        limit: int = 20,
        offset: int = 0,
    ) -> DLQEventsListResponse:
        """List and filter dead-letter queue (DLQ) failed events asynchronously.

        Args:
            source: Optional originating source subsystem ('sentiment', 'ingestion', 'options', 'fix').
            error_type: Optional error classification filter.
            status: Optional lifecycle status filter ('failed', 'retrying', 'reprocessed', 'purged', 'all', default: 'failed').
            start_date: Optional start date boundary in YYYY-MM-DD format.
            end_date: Optional end date boundary in YYYY-MM-DD format.
            limit: Maximum number of events to return per page (1..200, default: 20).
            offset: Pagination offset (default: 0).

        Returns:
            DLQEventsListResponse with paginated DLQ event summaries.
        """
        await self._ensure_authenticated()

        params: Dict[str, Any] = {
            "limit": limit,
            "offset": offset,
        }
        if source:
            params["source"] = source.strip()
        if error_type:
            params["error_type"] = error_type.strip()
        if status:
            params["status"] = status.strip()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/dlq/events", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DLQEventsListResponse.model_validate(resp.json())

    async def get_dlq_event(
        self,
        id: str,
    ) -> DLQEventDetail:
        """Retrieve full details and un-truncated payload of a DLQ event asynchronously.

        Args:
            id: Unique UUID identifier string of the DLQ record.

        Returns:
            DLQEventDetail containing full payload and error metadata.
        """
        await self._ensure_authenticated()

        clean_id = id.strip()
        if not clean_id:
            raise FinTextValidationError("DLQ event 'id' cannot be empty.")

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get(f"/dlq/events/{clean_id}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DLQEventDetail.model_validate(resp.json())

    async def reprocess_dlq_event(
        self,
        id: str,
    ) -> ReprocessDLQResponse:
        """Trigger immediate reprocessing for a failed DLQ event asynchronously.

        Args:
            id: Unique UUID identifier string of the DLQ record to reprocess.

        Returns:
            ReprocessDLQResponse confirming reprocessing initiation.
        """
        await self._ensure_authenticated()

        clean_id = id.strip()
        if not clean_id:
            raise FinTextValidationError("DLQ event 'id' cannot be empty.")

        headers = {
            "Authorization": f"Bearer {self.api_token}",
            "Content-Type": "application/json",
        }

        try:
            resp = await self._client.post(f"/dlq/events/{clean_id}/reprocess", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ReprocessDLQResponse.model_validate(resp.json())

    async def purge_dlq_event(
        self,
        id: str,
    ) -> PurgeDLQResponse:
        """Permanently purge a DLQ event from quarantine asynchronously.

        Args:
            id: Unique UUID identifier string of the DLQ record to purge.

        Returns:
            PurgeDLQResponse confirming event purge.
        """
        await self._ensure_authenticated()

        clean_id = id.strip()
        if not clean_id:
            raise FinTextValidationError("DLQ event 'id' cannot be empty.")

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.delete(f"/dlq/events/{clean_id}", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PurgeDLQResponse.model_validate(resp.json())

    async def sla_status(
        self,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        percentiles: Optional[str] = "50,95,99",
        sla_target_ms: int = 100,
    ) -> SLAStatusResponse:
        """Retrieve API latency percentiles and contractual SLA compliance performance report asynchronously.

        Args:
            start_date: Optional start date boundary in YYYY-MM-DD format (defaults to 30 days prior).
            end_date: Optional end date boundary in YYYY-MM-DD format (defaults to current date).
            percentiles: Comma-separated list of latency percentiles to calculate (default: '50,95,99').
            sla_target_ms: Contractual latency SLA target in milliseconds (10..5000, default: 100).

        Returns:
            SLAStatusResponse containing request volumes, average latency, exact percentiles, and SLA compliance status.
        """
        await self._ensure_authenticated()

        params: Dict[str, Any] = {
            "sla_target_ms": sla_target_ms,
        }
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()
        if percentiles:
            params["percentiles"] = percentiles.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sla/status", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SLAStatusResponse.model_validate(resp.json())

    async def sla_latency(
        self,
        ticker: Optional[str] = None,
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
        percentiles: Optional[str] = "50,95,99",
        sla_target_ms: int = 500,
    ) -> SLALatencyResponse:
        """Retrieve signal ingestion and processing pipeline latency metrics and SLA compliance report asynchronously.

        Measures point-in-time latency across all five signal processing stages: source publication ->
        fetch -> normalization -> ONNX inference -> database commit (signal_available_ts - source_event_ts).

        Args:
            ticker: Optional ticker symbol filter (e.g. 'AAPL', 'NVDA'). If omitted, aggregates universe-wide.
            start_date: Optional start date boundary in YYYY-MM-DD format (defaults to 30 days prior).
            end_date: Optional end date boundary in YYYY-MM-DD format (defaults to current date).
            percentiles: Comma-separated list of latency percentiles to calculate (default: '50,95,99').
            sla_target_ms: Target signal latency SLA in milliseconds (10..10000, default: 500).

        Returns:
            SLALatencyResponse containing total signal counts, average freshness latency, exact percentiles,
            stage breakdown, and SLA compliance status.
        """
        await self._ensure_authenticated()

        params: Dict[str, Any] = {
            "sla_target_ms": sla_target_ms,
        }
        if ticker:
            params["ticker"] = ticker.strip()
        if start_date:
            params["start_date"] = start_date.strip()
        if end_date:
            params["end_date"] = end_date.strip()
        if percentiles:
            params["percentiles"] = percentiles.strip()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sla/latency", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SLALatencyResponse.model_validate(resp.json())

    async def activate_sandbox(self) -> SandboxStatusResponse:
        """
        Activate Sandbox simulation mode for the authenticated user session asynchronously.

        Subsequent API calls will be evaluated against isolated mock data models,
        preventing consumption of production billing quotas or pollution of operational telemetry.

        Returns:
            SandboxStatusResponse detailing active sandbox status and available mock endpoints.
        """
        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/sandbox/activate", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SandboxStatusResponse.model_validate(resp.json())

    async def deactivate_sandbox(self) -> SandboxStatusResponse:
        """
        Deactivate Sandbox simulation mode, reverting the user session back to live production pipelines asynchronously.

        Returns:
            SandboxStatusResponse confirming deactivation.
        """
        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/sandbox/deactivate", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SandboxStatusResponse.model_validate(resp.json())

    async def get_sandbox_status(self) -> SandboxStatusResponse:
        """
        Get the current Sandbox status, mock data catalog version, and supported mock endpoints asynchronously.

        Returns:
            SandboxStatusResponse with active status and configuration metadata.
        """
        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/sandbox/status", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SandboxStatusResponse.model_validate(resp.json())

    async def get_provenance(
        self,
        record_type: str,
        record_id: str,
    ) -> DataProvenanceResponse:
        """
        Retrieve granular data provenance and processing lineage history for a specific record asynchronously.

        Args:
            record_type: Entity type of the record ('sentiment' or 'news').
            record_id: Unique record identifier (e.g. 'AAPL_2026-08-30T10:15:00Z' or article UUID).

        Returns:
            DataProvenanceResponse containing provenance audit items, model/pipeline tags, and execution steps.
        """
        if not record_type or not record_type.strip():
            raise FinTextValidationError("Field 'record_type' cannot be empty.")
        if not record_id or not record_id.strip():
            raise FinTextValidationError("Field 'record_id' cannot be empty.")

        norm_type = record_type.strip().lower()
        if norm_type not in ("sentiment", "news"):
            raise FinTextValidationError(f"Invalid record_type '{record_type}'. Allowed values: ['sentiment', 'news']")

        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}
        endpoint = f"/provenance/{norm_type}/{record_id.strip()}"

        try:
            resp = await self._client.get(endpoint, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return DataProvenanceResponse.model_validate(resp.json())

    async def trigger_anomaly_scan(self) -> AnomalyScanResponse:
        """
        Trigger an on-demand statistical sentiment anomaly scan and broadcast alerts to active WebSocket clients.

        Returns:
            AnomalyScanResponse containing detected anomaly alerts and broadcast counts.
        """
        await self._ensure_authenticated()

        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.post("/anomaly-scan", headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return AnomalyScanResponse.model_validate(resp.json())

    def get_anomaly_websocket_url(self, token: Optional[str] = None, format: str = "json") -> str:
        """
        Construct the WebSocket streaming URL configured for real-time sentiment anomaly alerts.

        Args:
            token: Optional JWT token string. Defaults to the client's current api_token.
            format: Streaming serialization format ("json" or "msgpack"). Defaults to "json".

        Returns:
            Full WebSocket connection URL (e.g. 'ws://127.0.0.1:8000/ws?token=...&streams=anomalies&format=msgpack').
        """
        auth_token = token or self.api_token or ""
        base = str(self._client.base_url).rstrip("/")
        ws_base = base.replace("http://", "ws://").replace("https://", "wss://")
        params = ["streams=anomalies"]
        if auth_token:
            params.insert(0, f"token={auth_token}")
        if format and format.strip().lower() in ("msgpack", "messagepack", "mp"):
            params.append("format=msgpack")

        query_str = "&".join(params)
        return f"{ws_base}/ws?{query_str}"

    async def detect_language(
        self,
        text: str,
    ) -> LanguageDetectionResponse:
        """
        Detect the language of input text and return the assigned multilingual model routing.

        Uses statistical stopword analysis and Unicode script recognition to classify
        the language and select the appropriate inference model.

        Args:
            text: Input text to analyze (headline, financial document, or transcript excerpt).

        Returns:
            LanguageDetectionResponse with detected language, confidence, and model routing metadata.
        """
        if not text or not text.strip():
            raise FinTextValidationError("Field 'text' cannot be empty.")

        await self._ensure_authenticated()

        params = {"text": text.strip()}
        headers = {"Authorization": f"Bearer {self.api_token}"}

        try:
            resp = await self._client.get("/language/detect", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return LanguageDetectionResponse.model_validate(resp.json())

    async def get_model_card(self) -> ModelCardResponse:
        """
        Retrieve the standardized Model Card and Lineage Governance Report asynchronously.

        Returns details regarding neural architecture, base foundation checkpoint,
        fine-tuning dataset, quantization, precision, sequence length, latency benchmarks,
        hardware specifications, version history, and intellectual property licensing.

        Returns:
            ModelCardResponse detailing full institutional model governance metadata.
        """
        try:
            resp = await self._client.get("/model-card")
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        return ModelCardResponse.model_validate(resp.json())

    # Ergonomic alias
    model_card = get_model_card

    async def generate_alpha_report(
        self,
        tickers: list[str],
        start_date: str,
        end_date: str,
        signal_config: Optional[AlphaSignalConfig] = None,
        benchmark_ticker: Optional[str] = "SPY",
        initial_capital: Optional[float] = 1_000_000.0,
    ) -> AlphaReportResponse:
        """
        Generate an Alpha Signal Validation Backtest Report asynchronously.

        Evaluates the historical performance of a simple sentiment-driven
        trading strategy across the specified ticker universe, returning
        institutional-grade risk & return attribution metrics (Sharpe, Sortino,
        max drawdown, alpha, beta, information ratio) and a complete daily
        equity curve with benchmark comparison.

        Args:
            tickers: List of target constituent ticker symbols (1 to 10).
            start_date: Start date for historical evaluation (YYYY-MM-DD).
            end_date: End date for historical evaluation (YYYY-MM-DD).
            signal_config: Strategy signal generation parameters.
                Defaults to sentiment with threshold_long=0.2, threshold_short=-0.2, holding_days=5.
            benchmark_ticker: Benchmark asset ticker for relative performance (default: "SPY").
            initial_capital: Initial portfolio capital balance in USD (default: $1,000,000).

        Returns:
            AlphaReportResponse with performance metrics and equity curve.

        Raises:
            FinTextValidationError: If tickers list is empty or exceeds 10 symbols.
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        if not tickers or len(tickers) == 0:
            raise FinTextValidationError("Field 'tickers' must contain at least one ticker symbol.")
        if len(tickers) > 10:
            raise FinTextValidationError("Field 'tickers' must contain at most 10 ticker symbols.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        config = signal_config or AlphaSignalConfig()
        payload = {
            "tickers": [t.strip().upper() for t in tickers],
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
            "signal_config": config.model_dump(exclude_none=True),
        }
        if benchmark_ticker is not None:
            payload["benchmark_ticker"] = benchmark_ticker.strip()
        if initial_capital is not None:
            payload["initial_capital"] = initial_capital

        try:
            resp = await self._client.post("/signals/alpha-report", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return AlphaReportResponse.model_validate(resp.json())

    # Ergonomic alias
    alpha_report = generate_alpha_report

    async def pit_replay(
        self,
        ticker: str,
        as_of_utc: str,
        include_news: bool = True,
        include_filings: bool = True,
        include_events: bool = True,
        include_sentiment: bool = True,
        limit: Optional[int] = 50,
    ) -> PITReplayResponse:
        """
        Reconstruct Point-in-Time Historical State & Look-Ahead Bias Verification.

        Retrieves the exact dataset committed to the database on or before `as_of_utc`,
        reconstructing the model's historical knowledge state across news articles,
        SEC filings, corporate events, and sentiment scores.

        Args:
            ticker: Target stock ticker symbol (e.g. "AAPL", "NVDA").
            as_of_utc: Historical as-of point-in-time timestamp (RFC3339 format).
            include_news: Whether to include news articles committed as of timestamp (default: True).
            include_filings: Whether to include SEC regulatory filings (default: True).
            include_events: Whether to include corporate events (default: True).
            include_sentiment: Whether to include sentiment analysis records (default: True).
            limit: Maximum number of records per category (default: 50, max: 200).

        Returns:
            PITReplayResponse with reconstructed entities and temporal consistency audit.

        Raises:
            FinTextValidationError: If ticker or as_of_utc is empty or limit is invalid.
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        if not ticker or not ticker.strip():
            raise FinTextValidationError("Parameter 'ticker' must not be empty.")
        if not as_of_utc or not as_of_utc.strip():
            raise FinTextValidationError("Parameter 'as_of_utc' must not be empty.")
        if limit is not None and (limit <= 0 or limit > 200):
            raise FinTextValidationError("Parameter 'limit' must be between 1 and 200.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        params: dict[str, Any] = {
            "ticker": ticker.strip().upper(),
            "as_of_utc": as_of_utc.strip(),
            "include_news": include_news,
            "include_filings": include_filings,
            "include_events": include_events,
            "include_sentiment": include_sentiment,
        }
        if limit is not None:
            params["limit"] = limit

        try:
            resp = await self._client.get("/pit/replay", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PITReplayResponse.model_validate(resp.json())

    # Ergonomic alias
    replay = pit_replay

    async def signal_quality_report(
        self,
        signal_type: str,
        tickers: list[str],
        start_date: str,
        end_date: str,
        horizon_days: Optional[int] = 5,
        benchmark_ticker: Optional[str] = "SPY",
    ) -> SignalQualityReportResponse:
        """
        Evaluate Historical Signal Quality, Information Coefficient (IC), Decay, and Bias.

        Analyzes coverage, freshness, Spearman Rank IC against forward returns,
        holding horizon decay trajectory, hit rate, and systematic sector/cap bias.

        Args:
            signal_type: Signal stream family ('sentiment', 'spillover', 'gex', 'insider', 'event').
            tickers: List of constituent stock ticker symbols (1 to 20).
            start_date: Backtest evaluation start date (YYYY-MM-DD).
            end_date: Backtest evaluation end date (YYYY-MM-DD).
            horizon_days: Forward return horizon in trading days (default: 5, range: 1-20).
            benchmark_ticker: Benchmark asset ticker for relative comparison (default: "SPY").

        Returns:
            SignalQualityReportResponse containing IC, decay curves, half-life, and bias breakdowns.

        Raises:
            FinTextValidationError: If parameters are invalid.
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        if not signal_type or not signal_type.strip():
            raise FinTextValidationError("Parameter 'signal_type' must not be empty.")
        if not tickers or len(tickers) == 0:
            raise FinTextValidationError("Field 'tickers' must contain at least one ticker symbol.")
        if len(tickers) > 20:
            raise FinTextValidationError("Field 'tickers' must contain at most 20 ticker symbols.")
        if not start_date or not start_date.strip():
            raise FinTextValidationError("Field 'start_date' must not be empty.")
        if not end_date or not end_date.strip():
            raise FinTextValidationError("Field 'end_date' must not be empty.")
        if horizon_days is not None and (horizon_days < 1 or horizon_days > 20):
            raise FinTextValidationError("Field 'horizon_days' must be between 1 and 20.")

        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        payload: dict[str, Any] = {
            "signal_type": signal_type.strip().lower(),
            "tickers": [t.strip().upper() for t in tickers],
            "start_date": start_date.strip(),
            "end_date": end_date.strip(),
        }
        if horizon_days is not None:
            payload["horizon_days"] = horizon_days
        if benchmark_ticker is not None:
            payload["benchmark_ticker"] = benchmark_ticker.strip().upper()

        try:
            resp = await self._client.post("/signals/quality-report", json=payload, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return SignalQualityReportResponse.model_validate(resp.json())

    # Ergonomic alias
    quality_report = signal_quality_report

    async def pit_certificate(
        self,
        dataset_version: Optional[str] = "2.1.0",
        universe: Optional[str] = "all",
        start_date: Optional[str] = None,
        end_date: Optional[str] = None,
    ) -> PITCertificateResponse:
        """
        Obtain a formal Cryptographically Signed Point-in-Time & Look-Ahead Bias Certificate.

        Executes 8 automated tests certifying temporal ordering, ticker renames,
        delisted securities, corporate actions, event duplication, clock skew,
        and backfill policies.

        Args:
            dataset_version: Dataset or processing pipeline version to certify (default: '2.1.0').
            universe: Target stock universe filter ('all', 'sp500', or comma-separated tickers, default: 'all').
            start_date: Historical audit start date in YYYY-MM-DD format (default: 90 days ago).
            end_date: Historical audit end date in YYYY-MM-DD format (default: today).

        Returns:
            PITCertificateResponse containing audit test results, policies, and SHA-256 signature.

        Raises:
            FinTextValidationError: If parameters are invalid.
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        params: dict[str, Any] = {}
        if dataset_version is not None:
            params["dataset_version"] = dataset_version.strip()
        if universe is not None:
            params["universe"] = universe.strip()
        if start_date is not None:
            params["start_date"] = start_date.strip()
        if end_date is not None:
            params["end_date"] = end_date.strip()

        try:
            resp = await self._client.get("/pit/certificate", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return PITCertificateResponse.model_validate(resp.json())

    # Ergonomic alias
    certify_pit = pit_certificate

    async def get_provider_health(
        self,
        provider: str = "all",
        window_minutes: int = 60,
    ) -> ProviderHealthResponse:
        """
        Query real-time operational health and performance metrics for upstream financial data providers.

        Parameters:
            provider: Target provider to filter ('sec_edgar', 'finnhub', 'polygon', 'all'). Default 'all'.
            window_minutes: Monitoring observation window in minutes (1 to 1440). Default 60.

        Returns:
            ProviderHealthResponse containing provider telemetry, success rates, latency, and status.

        Raises:
            FinTextValidationError: If provider or window_minutes is invalid.
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        params: dict[str, Any] = {
            "provider": provider.strip(),
            "window_minutes": window_minutes,
        }

        try:
            resp = await self._client.get("/providers/health", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ProviderHealthResponse.model_validate(resp.json())

    # Ergonomic alias
    provider_health = get_provider_health

    async def get_model_validation(
        self,
        recalibrate: bool = False,
        dataset_version: Optional[str] = None,
    ) -> ModelValidationResponse:
        """
        Query institutional FinBERT model validation report, multi-class metrics, and calibration curve.

        Parameters:
            recalibrate: Whether to trigger dynamic probability calibration computation (default False).
            dataset_version: Specific benchmark dataset version to evaluate (optional).

        Returns:
            ModelValidationResponse containing accuracy, macro F1, confusion matrix, and calibration curve.

        Raises:
            FinTextAuthenticationError: If not authenticated.
            FinTextConnectionError: If network connection fails.
        """
        await self._ensure_authenticated()
        headers = {"Authorization": f"Bearer {self.api_token}"}

        params: dict[str, Any] = {}
        if recalibrate:
            params["recalibrate"] = "true"
        if dataset_version:
            params["dataset_version"] = dataset_version.strip()

        try:
            resp = await self._client.get("/model-validation", params=params, headers=headers)
        except httpx.RequestError as exc:
            raise FinTextConnectionError(f"Failed to connect to FinText API: {exc}") from exc

        if not resp.is_success:
            self._handle_response_error(resp)

        self._update_rate_limit_headers(resp.headers)
        return ModelValidationResponse.model_validate(resp.json())

    # Ergonomic alias
    model_validation = get_model_validation

    async def close(self) -> None:
        """Close the underlying asynchronous HTTP client session."""
        await self._client.aclose()


    async def __aenter__(self) -> FinTextAsyncClient:
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.close()





