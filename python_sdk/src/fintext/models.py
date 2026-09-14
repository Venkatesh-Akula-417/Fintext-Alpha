"""
═══════════════════════════════════════════════════════════════════════════════
FinText Alpha Vectorizer Python Client SDK — Pydantic Data Models
═══════════════════════════════════════════════════════════════════════════════
"""

from typing import Any, Dict, List, Optional, Union
from pydantic import BaseModel, Field, ConfigDict


class HealthResponse(BaseModel):
    """Liveness probe and system health response."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="System operational status (e.g. 'ok')")
    version: str = Field(description="Production engine version string")
    timestamp_us: int = Field(description="Microsecond Unix timestamp of server")


class ModelMetadata(BaseModel):
    """Institutional model versioning and data provenance lineage metadata."""
    model_config = ConfigDict(extra="ignore")

    model_version: str = Field(default="finbert-v3.1.0", description="ML model version tag")
    pipeline_version: str = Field(default="2.0.0", description="Feature extraction pipeline version")
    data_provenance: list[str] = Field(default_factory=lambda: ["SEC EDGAR", "Finnhub", "Polygon"], description="Upstream source lineage")


class IssueTokenRequest(BaseModel):
    """Payload for issuing a temporary institutional JWT."""
    user_id: str = Field(description="User or institutional fund identifier")
    expires_in_seconds: Optional[int] = Field(default=3600, description="Token lifetime in seconds")
    role: Optional[str] = Field(default="institutional", description="Role tag (e.g., 'institutional')")


class IssueTokenResponse(BaseModel):
    """JWT token issuance response."""
    model_config = ConfigDict(extra="ignore")

    token: str = Field(description="Signed HMAC-SHA256 JWT string")
    token_type: str = Field(default="Bearer", description="Authorization header scheme")
    expires_in: int = Field(description="Duration in seconds until token expiration")
    user_id: str = Field(description="Target authenticated user identifier")
    role: str = Field(description="Assigned role string")


class SentimentProbabilities(BaseModel):
    """Class probabilities distribution across positive, neutral, and negative sentiment."""
    model_config = ConfigDict(extra="ignore")

    positive: float = Field(default=0.0, description="Positive sentiment probability (0.0 to 1.0)")
    neutral: float = Field(default=0.0, description="Neutral sentiment probability (0.0 to 1.0)")
    negative: float = Field(default=0.0, description="Negative sentiment probability (0.0 to 1.0)")


class SentimentResponse(BaseModel):
    """Point-in-time financial sentiment response from QuestDB."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Stock asset ticker symbol")
    date: str = Field(description="Query date formatted as YYYY-MM-DD or 'LATEST'")
    sentiment_score: float = Field(description="Normalized sentiment score (-1.0 to +1.0)")
    sentiment_label: str = Field(description="Classification label ('BULLISH', 'BEARISH', 'NEUTRAL')")
    confidence: float = Field(default=0.0, description="Model prediction confidence score (0.0 to 1.0)")
    probabilities: Optional[SentimentProbabilities] = Field(default=None, description="Detailed class probabilities distribution")
    signal_available_ts_us: int = Field(description="Point-in-time timestamp in microseconds")
    data_quality_score: float = Field(default=0.80, description="Quantitative data quality and reliability score (0.0 to 1.0)")
    message: str = Field(description="Diagnostic or operational message")
    model_version: Optional[str] = Field(default=None, description="ML model version tag (e.g., 'finbert-v3.1.0')")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version (e.g., '2.0.0')")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream source lineage data providers")
    published_utc: Optional[str] = Field(default=None, description="Original publication timestamp in ISO-8601 UTC")
    ingested_utc: Optional[str] = Field(default=None, description="System ingestion timestamp in ISO-8601 UTC")
    db_commit_utc: Optional[str] = Field(default=None, description="Database commit timestamp in ISO-8601 UTC")
    valid_from: Optional[str] = Field(default=None, description="Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)")
    valid_to: Optional[str] = Field(default=None, description="Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)")
    revision_number: Optional[int] = Field(default=None, description="Slowly Changing Dimension revision number (starts at 1)")
    is_current: Optional[bool] = Field(default=None, description="True if this record represents the latest active revision")


class SpilloverItem(BaseModel):
    """Individual cross-asset spillover correlation relation."""
    model_config = ConfigDict(extra="ignore")

    related_ticker: str = Field(description="Associated correlated asset ticker")
    lag_hours: int = Field(description="Lead-lag offset in hours (positive = leading)")
    correlation: float = Field(description="Pearson cross-correlation coefficient")
    relationship: str = Field(description="Human-readable description of the relationship")
    updated_at: str = Field(description="ISO-8601 calculation timestamp")


class SpilloverResponse(BaseModel):
    """Cross-asset spillover network query response."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Query asset ticker symbol")
    spillovers: list[SpilloverItem] = Field(default_factory=list, description="Ranked list of spillovers")
    count: int = Field(description="Number of spillover items returned")
    status: str = Field(description="Query status ('ok')")
    message: str = Field(description="Diagnostic message")


class SpilloverMatrixItem(BaseModel):
    """Pairwise cross-asset lead-lag spillover relationship in a correlation matrix."""
    model_config = ConfigDict(extra="ignore")

    ticker_a: str = Field(description="Primary asset ticker symbol")
    ticker_b: str = Field(description="Associated correlated asset ticker symbol")
    correlation: float = Field(description="Pearson cross-correlation coefficient")
    lag_hours: int = Field(description="Optimal lead-lag offset in hours (positive = A leads B; negative = B leads A)")
    direction: str = Field(description="Lead-lag direction tag (e.g. 'AAPL_leads_MSFT', 'self')")


class SpilloverMatrixResponse(BaseModel):
    """Cross-asset spillover correlation matrix query response."""
    model_config = ConfigDict(extra="ignore")

    tickers: list[str] = Field(description="List of analyzed stock ticker symbols")
    start_date: str = Field(description="Analysis start date (YYYY-MM-DD)")
    end_date: str = Field(description="Analysis end date (YYYY-MM-DD)")
    min_correlation: float = Field(description="Minimum correlation filter threshold applied")
    max_lag_hours: int = Field(description="Maximum lead-lag window in hours applied")
    count: int = Field(description="Number of matrix items returned")
    matrix: list[SpilloverMatrixItem] = Field(default_factory=list, description="Pairwise lead-lag correlation matrix items")
    generated_at: str = Field(description="ISO-8601 generation timestamp")



class EquityPoint(BaseModel):
    """Daily portfolio equity progression point."""
    model_config = ConfigDict(extra="ignore")

    date: str = Field(description="Valuation date (YYYY-MM-DD)")
    portfolio_value: float = Field(description="Portfolio cash valuation in USD")
    daily_return: float = Field(default=0.0, description="Daily percentage return")


class BacktestRequest(BaseModel):
    """Quantitative simulation parameters for alpha strategy backtesting."""
    ticker: Optional[str] = Field(default=None, description="Target asset ticker symbol (legacy single ticker)")
    tickers: Optional[list[str]] = Field(default=None, description="Multi-asset portfolio tickers (1 to 10)")
    weights: Optional[list[float]] = Field(default=None, description="Portfolio weights per ticker (sum should equal 1.0)")
    benchmark_ticker: Optional[str] = Field(default="SPY", description="Benchmark asset ticker symbol")
    transaction_cost_bps: Optional[float] = Field(default=5.0, description="Transaction fee in basis points (0-100 bps)")
    start_date: str = Field(description="Simulation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Simulation end date (YYYY-MM-DD)")
    long_threshold: float = Field(default=0.2, description="Sentiment threshold to enter LONG position")
    short_threshold: float = Field(default=-0.2, description="Sentiment threshold to enter SHORT position")
    holding_days: int = Field(default=5, description="Holding duration in trading days")
    initial_capital: float = Field(default=1_000_000.0, description="Starting cash capital in USD")


class BacktestResponse(BaseModel):
    """Quantitative alpha strategy backtest performance results."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Asset ticker symbol or portfolio summary")
    tickers: list[str] = Field(default_factory=list, description="Evaluated portfolio tickers")
    weights: list[float] = Field(default_factory=list, description="Evaluated portfolio weights")
    benchmark_ticker: str = Field(default="SPY", description="Benchmark ticker symbol")
    start_date: str = Field(description="Backtest start date")
    end_date: str = Field(description="Backtest end date")
    total_return: float = Field(description="Net cumulative portfolio return")
    annualized_return: float = Field(description="Annualized return percentage")
    sharpe_ratio: float = Field(description="Annualized Sharpe ratio")
    sortino_ratio: float = Field(default=0.0, description="Annualized Sortino ratio")
    max_drawdown: float = Field(description="Maximum peak-to-trough drawdown")
    num_trades: int = Field(description="Total executed round-trip trade positions")
    win_rate: float = Field(description="Percentage of winning trades (0.0 - 100.0)")
    profit_factor: float = Field(default=1.0, description="Gross profit / gross loss ratio")
    transaction_cost_bps: float = Field(default=5.0, description="Transaction fee in bps")
    equity_curve: list[float] = Field(default_factory=list, description="Daily portfolio equity progression")
    equity_points: list[EquityPoint] = Field(default_factory=list, description="Timestamped daily equity points")
    benchmark_curve: list[float] = Field(default_factory=list, description="Benchmark equity progression")
    benchmark_total_return: float = Field(default=0.0, description="Benchmark total return percentage")
    alpha: float = Field(default=0.0, description="Strategy excess return relative to benchmark")
    message: str = Field(description="Institutional backtest summary message")


class RateLimitInfo(BaseModel):
    """Rate limit quota metadata extracted from response headers."""
    limit: Optional[int] = Field(default=None, description="Max allowed requests per window")
    remaining: Optional[int] = Field(default=None, description="Remaining requests in window")
    reset: Optional[int] = Field(default=None, description="Seconds until bucket reset")


class SentimentRecord(BaseModel):
    """Single historical sentiment time series record with microstructure metrics."""
    model_config = ConfigDict(extra="ignore")

    published_utc: str = Field(description="Publication timestamp in ISO-8601 UTC")
    ticker: str = Field(description="Stock asset ticker symbol")
    source: str = Field(description="Data source or wire service")
    title: str = Field(description="Headline title or filing summary")
    sentiment_score: float = Field(description="Point-in-time sentiment score (-1.0 to +1.0)")
    vpin: float = Field(description="Volume-Synchronized Probability of Toxicity")
    gamma_exposure: float = Field(description="Dealer gamma exposure (GEX)")
    data_quality_score: float = Field(default=0.80, description="Quantitative data quality and reliability score (0.0 to 1.0)")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")
    ingested_utc: Optional[str] = Field(default=None, description="System ingestion timestamp in ISO-8601 UTC")
    db_commit_utc: Optional[str] = Field(default=None, description="Database commit timestamp in ISO-8601 UTC")
    valid_from: Optional[str] = Field(default=None, description="Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)")
    valid_to: Optional[str] = Field(default=None, description="Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)")
    revision_number: Optional[int] = Field(default=None, description="Slowly Changing Dimension revision number (starts at 1)")
    is_current: Optional[bool] = Field(default=None, description="True if this record represents the latest active revision")


class SentimentHistoryResponse(BaseModel):
    """Historical sentiment time series query response."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Queried asset ticker symbol")
    start_date: str = Field(description="Start date of queried window (YYYY-MM-DD)")
    end_date: str = Field(description="End date of queried window (YYYY-MM-DD)")
    count: int = Field(description="Number of records returned in this page")
    total: int = Field(description="Total number of records matching query")
    limit: int = Field(description="Limit applied")
    offset: int = Field(description="Offset applied")
    sort: str = Field(description="Sort direction ('asc' or 'desc')")
    records: list[SentimentRecord] = Field(default_factory=list, description="Array of historical sentiment records")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class RevisionIngestRequest(BaseModel):
    """Payload for creating a Slowly Changing Dimension Type 2 (SCD2) revision of a sentiment record."""
    ticker: str = Field(description="Target stock ticker symbol (e.g., 'AAPL', 'NVDA')")
    title: str = Field(description="Revised headline title or amendment filing summary")
    sentiment_score: float = Field(description="Revised point-in-time sentiment score (-1.0 to 1.0)")
    source: Optional[str] = Field(default="SEC EDGAR", description="Originating source or wire service")
    source_id: Optional[str] = Field(default=None, description="Optional natural key or filing accession identifier")
    ingested_utc: Optional[str] = Field(default=None, description="Ingestion timestamp in ISO-8601 UTC")
    db_commit_utc: Optional[str] = Field(default=None, description="Database commit timestamp in ISO-8601 UTC")


class RevisionIngestResponse(BaseModel):
    """Response payload after applying an SCD Type 2 revision."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target stock ticker symbol")
    message: str = Field(description="Summary of the SCD2 operation")
    active_revision: SentimentRecord = Field(description="The newly created active revision")
    superseded_revision: Optional[SentimentRecord] = Field(default=None, description="The previous revision that was superseded")


class RevisionHistoryListResponse(BaseModel):
    """Response payload containing full ordered revision history lineage for an asset."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target stock ticker symbol")
    total_revisions: int = Field(description="Total count of revision versions")
    revisions: list[SentimentRecord] = Field(default_factory=list, description="Complete ordered lineage of revision snapshots")


class SentimentFeedItem(BaseModel):
    """Consolidated real-time news sentiment signal feed item with microstructure metrics and data quality scoring."""
    model_config = ConfigDict(extra="ignore")

    published_utc: str = Field(description="Publication timestamp in ISO-8601 UTC")
    ticker: str = Field(description="Stock asset ticker symbol")
    source: str = Field(description="News or corporate disclosure source")
    title: str = Field(description="Headline title or filing summary")
    sentiment_score: float = Field(description="Point-in-time sentiment score (-1.0 to +1.0)")
    sentiment_label: str = Field(description="Classification label ('BULLISH', 'BEARISH', 'NEUTRAL')")
    confidence: float = Field(default=0.0, description="Model prediction confidence score (0.0 to 1.0)")
    data_quality_score: float = Field(default=0.80, description="Quantitative data quality and reliability score (0.0 to 1.0)")
    vpin: float = Field(default=0.0, description="Volume-Synchronized Probability of Toxicity (VPIN)")
    gamma_exposure: float = Field(default=0.0, description="Dealer gamma exposure (GEX)")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")
    ingested_utc: Optional[str] = Field(default=None, description="System ingestion timestamp in ISO-8601 UTC")
    db_commit_utc: Optional[str] = Field(default=None, description="Database commit timestamp in ISO-8601 UTC")
    valid_from: Optional[str] = Field(default=None, description="Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)")
    valid_to: Optional[str] = Field(default=None, description="Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)")
    revision_number: Optional[int] = Field(default=None, description="Slowly Changing Dimension revision number (starts at 1)")
    is_current: Optional[bool] = Field(default=None, description="True if this record represents the latest active revision")


class SentimentFeedResponse(BaseModel):
    """Response payload for consolidated news sentiment aggregated feed query."""
    model_config = ConfigDict(extra="ignore")

    count: int = Field(description="Number of returned records in the current page")
    total: int = Field(description="Total number of available records matching query filters")
    limit: int = Field(description="Page limit applied")
    offset: int = Field(description="Page offset applied")
    next_cursor: Optional[str] = Field(default=None, description="Cursor for keyset pagination (RFC3339 timestamp of last item in page)")
    sort: str = Field(description="Applied sort order ('asc' or 'desc')")
    sector: Optional[str] = Field(default=None, description="Optional filtered GICS sector")
    start_time: str = Field(description="Start time boundary of the feed window in ISO-8601 UTC")
    end_time: str = Field(description="End time boundary of the feed window in ISO-8601 UTC")
    min_confidence: float = Field(default=0.0, description="Minimum confidence threshold applied")
    min_quality: float = Field(default=0.0, description="Minimum data quality threshold applied")
    records: list[SentimentFeedItem] = Field(default_factory=list, description="Array of consolidated sentiment feed items")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class SentimentAnomalyItem(BaseModel):
    """Statistically significant sentiment anomaly record for an equity ticker."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Stock asset ticker symbol")
    latest_score: float = Field(description="Most recent point-in-time sentiment score (-1.0 to +1.0)")
    mean_score: float = Field(description="Baseline historical mean sentiment score")
    stddev: float = Field(description="Baseline historical sample standard deviation")
    zscore: float = Field(description="Computed z-score deviation relative to baseline")
    direction: str = Field(description="Anomaly classification direction: 'bullish' or 'bearish'")
    latest_timestamp: str = Field(description="Publication timestamp of most recent sentiment observation in ISO-8601 UTC")
    record_count: int = Field(description="Total number of historical observations evaluated in baseline")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class SentimentAnomaliesResponse(BaseModel):
    """Response payload for statistical sentiment anomaly detection scan."""
    model_config = ConfigDict(extra="ignore")

    count: int = Field(description="Number of returned anomaly items in current response")
    total_anomalies_detected: int = Field(description="Total number of detected anomalies matching criteria before limit")
    lookback_days: int = Field(description="Evaluated historical lookback window in calendar days")
    zscore_threshold: float = Field(description="Applied z-score threshold")
    min_records: int = Field(description="Applied minimum records filter threshold")
    sector: Optional[str] = Field(default=None, description="Optional filtered GICS sector")
    scanned_tickers: int = Field(description="Total number of active tickers scanned")
    items: list[SentimentAnomalyItem] = Field(default_factory=list, description="Array of detected sentiment anomaly items ranked by absolute z-score descending")
    generated_at: str = Field(description="UTC timestamp of calculation")
    message: str = Field(description="Operational summary message")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class SectorSentimentResponse(BaseModel):
    """Aggregate GICS sector-level financial sentiment metrics."""
    model_config = ConfigDict(extra="ignore")

    sector: str = Field(description="Target evaluated GICS sector name")
    start_date: str = Field(description="Start date of queried window (YYYY-MM-DD)")
    end_date: str = Field(description="End date of queried window (YYYY-MM-DD)")
    aggregation: str = Field(description="Applied aggregation operator ('average', 'sum', 'count', 'median', 'weighted_average')")
    min_confidence: float = Field(default=0.0, description="Applied minimum confidence filter")
    value: float = Field(description="Aggregated sentiment metric value")
    record_count: int = Field(description="Total number of evaluated records")
    tickers_included: int = Field(description="Total number of constituent tickers included")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of calculation")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class BatchSentimentResponse(BaseModel):
    """Batch point-in-time financial sentiment response across multiple tickers."""
    model_config = ConfigDict(extra="ignore")

    count: int = Field(description="Total count of sentiment records returned")
    results: list[SentimentResponse] = Field(default_factory=list, description="Array of individual asset sentiment signals")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class SecurityIdentifiers(BaseModel):
    """Financial security identifiers across international standard schemes."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Stock asset ticker symbol (e.g. 'AAPL')")
    figi: Optional[str] = Field(default=None, description="Bloomberg Financial Instrument Global Identifier")
    cusip: Optional[str] = Field(default=None, description="CUSIP 9-character identifier")
    isin: Optional[str] = Field(default=None, description="ISIN 12-character identifier")


class SymbolMapResponse(BaseModel):
    """Cross-identifier mapping resolution response."""
    model_config = ConfigDict(extra="ignore")

    input_identifier: str = Field(description="Input query identifier")
    input_type: str = Field(description="Resolved input identifier format ('ticker', 'figi', 'cusip', 'isin')")
    output_type: str = Field(description="Requested output identifier format ('ticker', 'figi', 'cusip', 'isin', 'all')")
    result: SecurityIdentifiers = Field(description="Resolved security identifiers")


class Universe(BaseModel):
    """Custom user-defined security universe watchlist."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique Universe identifier UUID")
    user_id: str = Field(description="Owner user identifier")
    name: str = Field(description="Universe display name")
    tickers: list[str] = Field(description="List of security ticker symbols")
    created_at: str = Field(description="ISO-8601 creation timestamp")
    updated_at: str = Field(description="ISO-8601 last update timestamp")


class ListUniversesResponse(BaseModel):
    """Paginated list of custom universes."""
    model_config = ConfigDict(extra="ignore")

    universes: list[Universe] = Field(default_factory=list, description="Array of custom universes")
    count: int = Field(description="Total count of universes returned")


class DeleteUniverseResponse(BaseModel):
    """Response confirming deletion of a custom universe."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Deleted Universe identifier UUID")
    status: str = Field(description="Deletion status string (e.g. 'deleted')")
    message: str = Field(description="Human readable confirmation message")


class OptionContract(BaseModel):
    """Normalized institutional option contract metrics with Black-Scholes Greeks."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Option contract ticker (e.g. 'O:AAPL251219C00250000')")
    underlying_ticker: str = Field(description="Underlying equity symbol (e.g. 'AAPL')")
    expiration_date: str = Field(description="Expiration date formatted as YYYY-MM-DD")
    strike: float = Field(description="Option strike price in USD")
    option_type: str = Field(description="Option contract type ('CALL' or 'PUT')")
    bid: float = Field(description="Best bid price in USD")
    ask: float = Field(description="Best ask price in USD")
    last: float = Field(description="Last executed trade price in USD")
    volume: int = Field(default=0, description="Daily contract trading volume")
    open_interest: int = Field(default=0, description="Open interest (total outstanding contracts)")
    implied_volatility: float = Field(description="Solved Black-Scholes implied volatility (annualized decimal)")
    delta: float = Field(description="Option Delta (rate of price change with respect to spot)")
    gamma: float = Field(description="Option Gamma (rate of delta change with respect to spot)")
    theta: float = Field(description="Option Theta (daily time decay in dollars per calendar day)")
    vega: float = Field(description="Option Vega (dollar change per 1% change in volatility)")
    rho: float = Field(description="Option Rho (dollar change per 1% change in interest rate)")


class OptionsIvResponse(BaseModel):
    """Options Implied Volatility and Greeks query response."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying equity ticker symbol")
    expiration_date: str = Field(description="Option expiration date (YYYY-MM-DD)")
    underlying_price: float = Field(description="Current underlying spot price in USD")
    risk_free_rate: float = Field(default=0.05, description="Applied annualized risk-free rate")
    dividend_yield: float = Field(default=0.0, description="Applied annualized continuous dividend yield")
    count: int = Field(description="Total number of option contracts returned")
    contracts: list[OptionContract] = Field(default_factory=list, description="Array of option contracts with IV and Greeks")
    message: str = Field(description="Operational or diagnostic summary message")


class UnusualOptionItem(BaseModel):
    """A single options contract flagged for unusual trading activity."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Option contract ticker identifier (e.g. 'O:AAPL251219C00250000')")
    underlying_ticker: str = Field(description="Underlying equity symbol (e.g. 'AAPL')")
    expiration_date: str = Field(description="Expiration date formatted as YYYY-MM-DD")
    strike: float = Field(description="Option strike price in USD")
    option_type: str = Field(description="Option contract type ('CALL' or 'PUT')")
    volume: int = Field(default=0, description="Current session contract trading volume")
    open_interest: int = Field(default=0, description="Open interest (total outstanding open contracts)")
    avg_volume: float = Field(description="Historical average daily volume over the lookback window")
    volume_oi_ratio: float = Field(description="Trading volume divided by open interest (volume / max(OI, 1))")
    volume_zscore: float = Field(description="Volume standard deviations above historical mean")
    score: float = Field(description="Composite unusualness ranking score (volume_oi_ratio * volume_zscore)")
    timestamp: str = Field(description="Snapshot ISO 8601 UTC timestamp")


class UnusualOptionsResponse(BaseModel):
    """Response envelope for unusual options activity (UOA) scan."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying ticker filter applied ('ALL' if universe scan)")
    min_volume_oi_ratio: float = Field(default=2.0, description="Applied minimum volume/OI ratio threshold")
    min_volume: int = Field(default=100, description="Applied minimum absolute volume threshold")
    days: int = Field(default=1, description="Lookback window in trading days")
    count: int = Field(description="Number of unusual options contracts returned")
    items: list[UnusualOptionItem] = Field(default_factory=list, description="Ranked list of unusual options contracts")
    message: str = Field(description="Operational or diagnostic summary message")


class UsageStatsSummary(BaseModel):
    """Aggregated usage summary metrics across the requested time window."""
    model_config = ConfigDict(extra="ignore")

    total_requests: int = Field(description="Total number of API requests recorded in the window")
    successful_requests: int = Field(description="Number of successful requests (HTTP 200..=299)")
    failed_requests: int = Field(description="Number of failed requests (HTTP status >= 400)")
    rate_limited_requests: int = Field(description="Number of rate-limited requests (HTTP 429)")
    average_latency_ms: float = Field(description="Mean request processing latency in milliseconds")
    p95_latency_ms: float = Field(description="95th percentile request processing latency in milliseconds")
    max_latency_ms: float = Field(description="Maximum request processing latency in milliseconds")


class UsageGroupItem(BaseModel):
    """Usage metrics breakdown item grouped by day, endpoint, method, or status code."""
    model_config = ConfigDict(extra="ignore")

    key: str = Field(description="Group identifier key (e.g. '2026-08-28', '/sentiment', 'GET', '200')")
    count: int = Field(description="Total requests in this group")
    successful_requests: int = Field(description="Successful requests (2xx) in this group")
    failed_requests: int = Field(description="Failed requests (>= 400) in this group")
    rate_limited_requests: int = Field(description="Rate-limited requests (429) in this group")
    average_latency_ms: float = Field(description="Average latency in milliseconds for this group")
    p95_latency_ms: float = Field(description="95th percentile latency in milliseconds for this group")
    max_latency_ms: float = Field(description="Maximum latency in milliseconds in this group")


class UsageStatsResponse(BaseModel):
    """Response envelope for user API consumption and usage analytics."""
    model_config = ConfigDict(extra="ignore")

    user_id: str = Field(description="Authenticated user identifier")
    start_date: str = Field(description="Start date filter applied (YYYY-MM-DD)")
    end_date: str = Field(description="End date filter applied (YYYY-MM-DD)")
    group_by: str = Field(description="Grouping dimension applied ('day', 'endpoint', 'method', 'status_code')")
    summary: UsageStatsSummary = Field(description="Overall summary aggregates")
    breakdown: list[UsageGroupItem] = Field(default_factory=list, description="Array of grouped usage metric items")
    message: str = Field(description="Operational or diagnostic summary message")


class AbnormalReturnPoint(BaseModel):
    """A single day's abnormal return and cumulative abnormal return observation."""
    model_config = ConfigDict(extra="ignore")

    date: str = Field(description="Trading date in ISO YYYY-MM-DD format")
    day_offset: int = Field(description="Trading day offset relative to event date (e.g., -5, 0, +5)")
    actual_return: float = Field(description="Actual stock return on this date")
    benchmark_return: float = Field(description="Market benchmark return on this date")
    expected_return: float = Field(description="Model expected return (alpha + beta * r_benchmark)")
    abnormal_return: float = Field(description="Abnormal return (actual_return - expected_return)")
    cumulative_abnormal_return: float = Field(description="Cumulative abnormal return (CAR) from start of window")


class EventStudyResponse(BaseModel):
    """Response envelope for event study analysis and CAR trajectory."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    event_date: str = Field(description="Corporate event date (YYYY-MM-DD)")
    event_window: int = Field(description="Event window trading days before/after event")
    estimation_window: int = Field(description="Estimation window trading days for OLS market model")
    benchmark_ticker: str = Field(description="Market model benchmark ticker (e.g. 'SPY')")
    alpha: float = Field(description="OLS market model intercept (Alpha)")
    beta: float = Field(description="OLS market model slope (Beta)")
    r_squared: float = Field(description="Market model R^2 fit on estimation window")
    car_full_window: float = Field(description="Cumulative abnormal return over the full event window")
    car_pre_event: float = Field(description="Cumulative abnormal return for pre-event window [-W_e, -1]")
    car_post_event: float = Field(description="Cumulative abnormal return for post-event window [+1, +W_e]")
    count: int = Field(description="Number of observation points in event window")
    abnormal_returns: list[AbnormalReturnPoint] = Field(default_factory=list, description="Daily abnormal return trajectory")
    message: str = Field(description="Methodology and operational summary message")


class EightKFiling(BaseModel):
    """SEC Form 8-K unscheduled corporate disclosure event filing."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    filing_date: str = Field(description="Filing publication date in ISO YYYY-MM-DD format")
    form_type: str = Field(description="SEC form type (e.g. '8-K')")
    event_type: str = Field(description="Classified event type category (e.g. 'M&A', 'CEO Change', 'Earnings Warning', 'Bankruptcy')")
    description: str = Field(description="Summary narrative description of the event")
    items: list[str] = Field(default_factory=list, description="List of SEC Item codes triggered")
    accession_number: str = Field(description="Unique SEC EDGAR accession number")
    url: str = Field(description="Direct URL to SEC EDGAR filing document archive")


class EightKResponse(BaseModel):
    """Response envelope for SEC Form 8-K filing query and event classification."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Queried ticker symbol or 'ALL'")
    event_type: str = Field(description="Applied event type filter or 'ALL'")
    days: int = Field(description="Lookback calendar days window applied")
    count: int = Field(description="Total number of filings matching query")
    filings: list[EightKFiling] = Field(default_factory=list, description="Array of matching SEC Form 8-K filings")
    message: str = Field(description="Operational summary message")


class SupplyChainRiskItem(BaseModel):
    """A supply chain connected node with computed propagated risk."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Related entity ticker symbol")
    relationship_type: str = Field(description="Relationship type relative to parent ('supplier', 'customer', 'partner', 'competitor')")
    depth: int = Field(description="Graph hop distance / depth level from query root (1..=4)")
    dependency_strength: float = Field(description="Direct or compounded dependency weight [0.0, 1.0]")
    sentiment_score: float = Field(description="Base point-in-time sentiment score for this entity [-1.0, 1.0]")
    sentiment_label: str = Field(description="Categorical sentiment label ('POSITIVE', 'NEUTRAL', 'NEGATIVE')")
    confidence: float = Field(description="Model sentiment confidence [0.0, 1.0]")
    risk_score: float = Field(description="Calculated propagated risk score [0.0, 1.0]")
    event_risk_multiplier: float = Field(description="Corporate disclosure 8-K event risk multiplier (e.g. 1.0 - 2.0)")
    recent_events_count: int = Field(description="Number of recent 8-K disclosure events detected")
    recent_event_types: list[str] = Field(default_factory=list, description="Recent event categories")


class SupplyChainRiskResponse(BaseModel):
    """Response envelope for supply chain risk propagation analysis."""
    model_config = ConfigDict(extra="ignore")

    root_ticker: str = Field(description="Queried root entity ticker symbol")
    root_sentiment: float = Field(description="Direct point-in-time sentiment score for root entity [-1.0, 1.0]")
    as_of_date: str = Field(description="Point-in-time evaluation date in ISO YYYY-MM-DD format")
    max_depth: int = Field(description="Maximum graph hop depth applied")
    decay_factor: float = Field(description="Distance decay damping factor applied per hop")
    event_lookback_days: int = Field(description="Lookback window in days for corporate 8-K disclosure events")
    relationship_filter: str = Field(description="Relationship category filter applied or 'ALL'")
    total_nodes_evaluated: int = Field(description="Total connected supply chain entity nodes evaluated")
    composite_supply_chain_risk: float = Field(description="Composite aggregated supply chain risk index [0.0, 1.0]")
    risk_tier: str = Field(description="Risk categorization tier ('LOW', 'MODERATE', 'ELEVATED', 'CRITICAL')")
    nodes: list[SupplyChainRiskItem] = Field(default_factory=list, description="Array of connected supply chain node risk assessments")
    message: str = Field(description="Methodological summary and operational message")


class AcousticFeatures(BaseModel):
    """Acoustic prosody and vocal stress features extracted from spoken audio."""
    model_config = ConfigDict(extra="ignore")

    pitch_mean_hz: float = Field(description="Fundamental frequency (F0) mean in Hz")
    energy_rms: float = Field(description="Root-Mean-Square (RMS) amplitude energy level")
    pause_ratio: float = Field(description="Ratio of silent / hesitation duration to total speech duration (0.0 to 1.0)")


class AudioSentiment(BaseModel):
    """Spoken content sentiment classification metrics."""
    model_config = ConfigDict(extra="ignore")

    score: float = Field(description="Quantitative sentiment score ranging from -1.0 (Bearish) to +1.0 (Bullish)")
    label: str = Field(description="Sentiment categorical classification ('BULLISH', 'BEARISH', 'NEUTRAL')")
    confidence: float = Field(description="Model prediction confidence score (0.0 to 1.0)")


class AudioTranscriptionResponse(BaseModel):
    """Response payload for audio transcription and acoustic sentiment analysis."""
    model_config = ConfigDict(extra="ignore")

    transcription: str = Field(description="Transcribed spoken text")
    duration_seconds: float = Field(description="Total audio recording duration in seconds")
    acoustic_features: AcousticFeatures = Field(description="Extracted vocal acoustic stress and prosody features")
    sentiment: AudioSentiment = Field(description="Financial sentiment analysis of transcribed text")
    word_count: int = Field(description="Total word count in transcription")
    language: str = Field(description="Detected or configured spoken language code")
    transcript_id: Optional[str] = Field(default=None, description="Optional UUID identifier if stored in transcript database")


class TranscriptMetadata(BaseModel):
    """Metadata summary of an earnings call transcript."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique transcript UUID identifier")
    user_id: str = Field(description="Owner user identifier")
    ticker: str = Field(description="Stock ticker symbol (e.g. AAPL, NVDA)")
    quarter: Optional[int] = Field(default=None, description="Fiscal quarter (1-4)")
    year: Optional[int] = Field(default=None, description="Fiscal year (e.g. 2024)")
    call_date: Optional[str] = Field(default=None, description="Earnings call date (YYYY-MM-DD)")
    source: str = Field(default="manual", description="Transcript source (e.g. manual, whisper_asr, sec_edgar)")
    sentiment_score: Optional[float] = Field(default=None, description="Sentiment score (-1.0 to 1.0)")
    sentiment_label: Optional[str] = Field(default=None, description="Sentiment categorical label")
    confidence: Optional[float] = Field(default=None, description="Sentiment confidence score (0.0 to 1.0)")
    word_count: int = Field(default=0, description="Total word count in transcript text")
    created_at: str = Field(description="ISO 8601 creation timestamp")


class TranscriptResponse(BaseModel):
    """Complete earnings call transcript record including full text."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique transcript UUID identifier")
    user_id: str = Field(description="Owner user identifier")
    ticker: str = Field(description="Stock ticker symbol")
    quarter: Optional[int] = Field(default=None, description="Fiscal quarter (1-4)")
    year: Optional[int] = Field(default=None, description="Fiscal year")
    call_date: Optional[str] = Field(default=None, description="Earnings call date (YYYY-MM-DD)")
    transcript_text: str = Field(description="Full text body of earnings call transcript")
    source: str = Field(default="manual", description="Transcript source")
    sentiment_score: Optional[float] = Field(default=None, description="Sentiment score")
    sentiment_label: Optional[str] = Field(default=None, description="Sentiment categorical label")
    confidence: Optional[float] = Field(default=None, description="Sentiment confidence")
    word_count: int = Field(default=0, description="Total word count")
    created_at: str = Field(description="ISO 8601 creation timestamp")


class TranscriptListResponse(BaseModel):
    """Paginated list of earnings call transcript metadata."""
    model_config = ConfigDict(extra="ignore")

    total: int = Field(description="Total matching transcripts count across all pages")
    count: int = Field(description="Number of transcript items in current page")
    limit: int = Field(description="Pagination limit parameter")
    offset: int = Field(description="Pagination offset parameter")
    items: list[TranscriptMetadata] = Field(default_factory=list, description="Array of transcript metadata records")


class DeleteTranscriptResponse(BaseModel):
    """Confirmation payload for transcript deletion."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Deleted transcript UUID identifier")
    deleted: bool = Field(description="True if transcript was deleted successfully")
    message: str = Field(description="Operational status message")


class RegimeComponents(BaseModel):
    """Granular underlying components contributing to market regime synthesis."""
    model_config = ConfigDict(extra="ignore")

    sector_sentiments: dict[str, float] = Field(default_factory=dict, description="Average sentiment score per GICS sector")
    total_data_points: int = Field(description="Total number of raw sentiment records evaluated in the window")
    positive_sentiment_ratio: float = Field(description="Ratio of evaluated tickers with strictly positive sentiment")
    bullish_tickers_count: int = Field(default=0, description="Number of tickers with positive sentiment (> 0.0)")
    bearish_tickers_count: int = Field(default=0, description="Number of tickers with negative sentiment (< 0.0)")
    neutral_tickers_count: int = Field(default=0, description="Number of tickers with neutral sentiment (== 0.0)")


class MarketRegimeResponse(BaseModel):
    """Response envelope for macro market regime detection."""
    model_config = ConfigDict(extra="ignore")

    regime: str = Field(description="Classified market regime: Bullish, Bearish, Neutral, or High Volatility")
    confidence: float = Field(description="Statistical confidence score between 0.0 and 1.0")
    market_sentiment: float = Field(description="Composite cross-sector aggregate market sentiment score (-1.0 to 1.0)")
    breadth: float = Field(description="Market breadth: proportion of universe assets with positive sentiment (0.0 to 1.0)")
    avg_spillover_corr: float = Field(description="Average pairwise absolute spillover correlation across representative universe")
    volatility_proxy: float = Field(description="Proxy measure for market sentiment dispersion and implied volatility")
    lookback_days: int = Field(description="Number of lookback days utilized in evaluation")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of analysis synthesis")
    components: RegimeComponents = Field(description="Underlying sector sentiment and breadth components")


class ReturnCorrelationItem(BaseModel):
    """Pairwise daily return correlation entry."""
    model_config = ConfigDict(extra="ignore")

    ticker_a: str = Field(description="First ticker in the pairwise relation")
    ticker_b: str = Field(description="Second ticker in the pairwise relation")
    correlation: Optional[float] = Field(default=None, description="Pearson correlation coefficient between -1.0 and +1.0 (None if insufficient data)")
    periods: int = Field(description="Number of overlapping daily return periods evaluated")


class ReturnCorrelationResponse(BaseModel):
    """Response envelope for daily return correlation matrix."""
    model_config = ConfigDict(extra="ignore")

    tickers: list[str] = Field(default_factory=list, description="Evaluated ticker universe")
    start_date: str = Field(description="Applied start date (YYYY-MM-DD)")
    end_date: str = Field(description="Applied end date (YYYY-MM-DD)")
    min_periods: int = Field(description="Minimum overlapping periods required for valid correlation")
    matrix: list[ReturnCorrelationItem] = Field(default_factory=list, description="List of pairwise correlation records")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of matrix generation")


class PutCallRatioPoint(BaseModel):
    """Daily observation point for Put/Call Ratio time series."""
    model_config = ConfigDict(extra="ignore")

    date: str = Field(description="Trading date in YYYY-MM-DD format")
    put_volume: int = Field(description="Total traded put volume on this date")
    call_volume: int = Field(description="Total traded call volume on this date")
    put_open_interest: Optional[int] = Field(default=None, description="Total put open interest on this date")
    call_open_interest: Optional[int] = Field(default=None, description="Total call open interest on this date")
    ratio: Optional[float] = Field(default=None, description="Computed put/call ratio (None if call metric is 0)")


class PutCallRatioResponse(BaseModel):
    """Response envelope for options Put/Call Ratio endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Evaluated underlying ticker (None if market-wide)")
    start_date: str = Field(description="Observation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Observation end date (YYYY-MM-DD)")
    ratio_type: str = Field(description="Evaluated ratio metric: 'volume' or 'open_interest'")
    granularity: str = Field(description="Aggregation granularity: 'daily' or 'total'")
    points: Optional[list[PutCallRatioPoint]] = Field(default=None, description="Time series of daily points (if granularity == 'daily')")
    average_ratio: Optional[float] = Field(default=None, description="Mean put/call ratio across valid daily points")
    total_put_volume: Optional[int] = Field(default=None, description="Total aggregated put volume (if granularity == 'total')")
    total_call_volume: Optional[int] = Field(default=None, description="Total aggregated call volume (if granularity == 'total')")
    total_put_open_interest: Optional[int] = Field(default=None, description="Total aggregated put open interest (if granularity == 'total')")
    total_call_open_interest: Optional[int] = Field(default=None, description="Total aggregated call open interest (if granularity == 'total')")
    total_ratio: Optional[float] = Field(default=None, description="Aggregate put/call ratio across entire period (if granularity == 'total')")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")


class EarningsSurpriseItem(BaseModel):
    """Individual quantified earnings surprise event with sentiment displacement metrics."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    earnings_date: str = Field(description="SEC 8-K Item 2.02 earnings release date (YYYY-MM-DD)")
    pre_avg_sentiment: float = Field(description="Average baseline sentiment in the pre-event window")
    post_avg_sentiment: float = Field(description="Average reaction sentiment in the post-event window")
    surprise_score: float = Field(description="Sentiment displacement score (post_avg_sentiment - pre_avg_sentiment)")
    direction: str = Field(description="Direction classification ('positive' or 'negative')")
    pre_record_count: int = Field(description="Number of sentiment records analyzed in the pre-event window")
    post_record_count: int = Field(description="Number of sentiment records analyzed in the post-event window")


class EarningsSurpriseResponse(BaseModel):
    """Response envelope for the Earnings Surprise Tracker endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Target ticker filter applied (None if universe-wide)")
    start_date: str = Field(description="Observation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Observation end date (YYYY-MM-DD)")
    min_sentiment_shift: float = Field(description="Minimum absolute sentiment shift threshold applied")
    pre_days: int = Field(description="Pre-event baseline window in trading days")
    post_days: int = Field(description="Post-event reaction window in trading days")
    count: int = Field(description="Total number of surprise events returned")
    surprises: list[EarningsSurpriseItem] = Field(default_factory=list, description="Array of detected earnings surprise events")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")


class InsiderTradeItem(BaseModel):
    """Individual SEC Form 4 insider trading transaction with normalized signal score."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    insider_name: str = Field(description="Full legal name of the reporting corporate insider")
    insider_role: str = Field(description="Corporate insider role ('CEO', 'CFO', 'Director', 'Officer', '10% Owner')")
    transaction_type: str = Field(description="Classified transaction type ('purchase', 'sale', 'grant', 'exercise')")
    shares: int = Field(description="Number of shares involved in the transaction")
    price: float = Field(description="Execution price per share in USD")
    value: float = Field(description="Total gross transaction value in USD")
    filing_date: str = Field(description="SEC Form 4 filing date (YYYY-MM-DD)")
    signal_score: float = Field(description="Normalized directional conviction signal score (-1.0 to +1.0)")
    source: str = Field(default="SEC Form 4", description="Regulatory data origin")


class InsiderTradingResponse(BaseModel):
    """Response envelope for the Insider Trading Signal endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Target ticker filter applied (None if universe-wide)")
    transaction_type: str = Field(description="Transaction type filter applied")
    start_date: str = Field(description="Observation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Observation end date (YYYY-MM-DD)")
    min_shares: int = Field(description="Minimum shares threshold applied")
    min_signal_score: float = Field(description="Minimum signal score threshold applied")
    count: int = Field(description="Total number of matching insider transactions returned")
    trades: list[InsiderTradeItem] = Field(default_factory=list, description="Array of insider trading transactions")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")


class SourceBreakdown(BaseModel):
    """Breakdown of sentiment statistics for a specific news or filing data provider."""
    model_config = ConfigDict(extra="ignore")

    source: str = Field(description="Data source provider name (e.g. 'SEC EDGAR', 'Finnhub', 'Bloomberg')")
    mean: float = Field(description="Arithmetic mean sentiment score across records from this source")
    count: int = Field(description="Number of sentiment records originating from this source")


class SentimentDisagreementResponse(BaseModel):
    """Response envelope for the Sentiment Disagreement Index endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target equity ticker symbol")
    start_date: str = Field(description="Observation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Observation end date (YYYY-MM-DD)")
    aggregation: str = Field(description="Applied dispersion aggregation method ('stddev', 'iqr', 'mad')")
    disagreement_index: float = Field(description="Quantified sentiment disagreement/dispersion index across sources")
    mean_sentiment: float = Field(description="Overall arithmetic mean sentiment across all analyzed records")
    median_sentiment: float = Field(description="Overall median sentiment across all analyzed records")
    record_count: int = Field(description="Total number of sentiment records analyzed")
    source_count: int = Field(description="Number of distinct news/filing sources represented")
    sources_breakdown: list[SourceBreakdown] = Field(default_factory=list, description="Detailed per-source sentiment breakdown")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class VolSurfacePoint(BaseModel):
    """A slice of the volatility surface along a single expiration date across multiple strikes."""
    model_config = ConfigDict(extra="ignore")

    expiration: str = Field(description="Option expiration date formatted as YYYY-MM-DD")
    ivs: list[float] = Field(default_factory=list, description="Solved Black-Scholes implied volatilities corresponding 1-to-1 with strikes")


class OptionsVolSurfaceResponse(BaseModel):
    """Response envelope for the Options Volatility Surface endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target underlying equity ticker symbol")
    spot: float = Field(description="Underlying asset spot price in USD used for moneyness calculation")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")
    strikes: list[float] = Field(default_factory=list, description="Array of strike prices in USD (columns of 2D surface)")
    expirations: list[str] = Field(default_factory=list, description="Array of expiration dates in YYYY-MM-DD format (rows of 2D surface)")
    surface: list[VolSurfacePoint] = Field(default_factory=list, description="2D volatility surface grid rows")


class MARumorItem(BaseModel):
    """A single company flagged with quantified M&A rumor metrics and signals."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    rumor_score: float = Field(description="Composite M&A rumor conviction score (0.0 to 1.0)")
    sentiment_zscore: float = Field(description="Recent sentiment anomaly z-score deviation")
    keyword_hits: int = Field(description="Frequency count of M&A keywords in recent news and transcripts")
    recent_8k_ma_flag: bool = Field(description="Whether recent 8-K filings contain M&A material event items")
    supply_chain_related_tickers: list[str] = Field(default_factory=list, description="Related supplier and customer tickers")
    insider_net_buying: bool = Field(description="Whether recent Form 4 filings exhibit net insider accumulation")
    latest_news_title: str = Field(description="Headline title of the most recent significant news article or filing")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of detection")


class MARumorsResponse(BaseModel):
    """Response envelope for the M&A Rumor Detection endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Target ticker filter applied (None if universe scan)")
    min_rumor_score: float = Field(description="Applied minimum rumor score threshold")
    lookback_days: int = Field(description="Lookback window in calendar days")
    count: int = Field(description="Total number of flagged M&A rumor candidates returned")
    items: list[MARumorItem] = Field(default_factory=list, description="Array of flagged M&A rumor candidates")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")


class RegulatoryFilingItem(BaseModel):
    """A single SEC regulatory filing with classified event category and EDGAR metadata."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Underlying stock ticker symbol")
    form_type: str = Field(description="Official SEC form type (e.g. '10-K', '10-Q', '8-K', 'S-1')")
    filing_date: str = Field(description="SEC filing date (YYYY-MM-DD)")
    accession_number: str = Field(description="Official SEC EDGAR accession number")
    event_category: str = Field(description="Standardized event category classified from form type and disclosure")
    description: str = Field(description="Descriptive title or summary of filing")
    url: str = Field(description="Official SEC EDGAR filing document URL")


class RegulatoryFilingsResponse(BaseModel):
    """Response envelope for the Regulatory Filings Classifier endpoint."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Target ticker filter applied (None if universe scan)")
    form_type: Optional[str] = Field(default=None, description="Form type filter applied")
    event_category: Optional[str] = Field(default=None, description="Event category filter applied")
    start_date: str = Field(description="Observation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Observation end date (YYYY-MM-DD)")
    count: int = Field(description="Number of filings returned in this page")
    total_count: int = Field(description="Total number of matching filings across all pages")
    offset: int = Field(description="Pagination offset applied")
    limit: int = Field(description="Pagination limit applied")
    filings: list[RegulatoryFilingItem] = Field(default_factory=list, description="Array of classified regulatory filings")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of calculation")


# ─────────────────────────────────────────────────────────────────────────────
# Billing & Subscription Models
# ─────────────────────────────────────────────────────────────────────────────

class CheckoutResponse(BaseModel):
    """Response containing a Stripe Checkout Session URL."""
    model_config = ConfigDict(extra="ignore")

    checkout_url: str = Field(description="Full Stripe Checkout Session URL for payment")
    session_id: str = Field(description="Stripe Checkout Session identifier")


class PortalResponse(BaseModel):
    """Response containing a Stripe Customer Portal URL."""
    model_config = ConfigDict(extra="ignore")

    portal_url: str = Field(description="Full Stripe Customer Portal session URL")


class SubscriptionResponse(BaseModel):
    """Current subscription status and usage information."""
    model_config = ConfigDict(extra="ignore")

    plan_id: str = Field(description="Active plan identifier (free, pro_monthly, enterprise_monthly)")
    plan_name: str = Field(description="Human-readable plan display name")
    status: str = Field(description="Subscription status (active, past_due, cancelled, inactive)")
    monthly_request_limit: Optional[int] = Field(default=None, description="Monthly API request limit (null = unlimited)")
    current_usage: int = Field(default=0, description="Current month's API request count")
    current_period_start: Optional[str] = Field(default=None, description="Current billing period start (ISO 8601)")
    current_period_end: Optional[str] = Field(default=None, description="Current billing period end (ISO 8601)")
    stripe_customer_id: Optional[str] = Field(default=None, description="Masked Stripe customer ID")


# ─────────────────────────────────────────────────────────────────────────────
# Organization & Multi-User Team Access Models
# ─────────────────────────────────────────────────────────────────────────────

class Organization(BaseModel):
    """Summary of an organization and the user's role in it."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Organization UUID identifier")
    name: str = Field(description="Organization display name")
    created_by: str = Field(description="User ID of creator")
    created_at: str = Field(description="Creation timestamp (ISO 8601)")
    member_count: int = Field(default=1, description="Number of active members")
    my_role: str = Field(description="Current user's role in organization (admin, member, viewer)")


class OrganizationMember(BaseModel):
    """Organization member item."""
    model_config = ConfigDict(extra="ignore")

    user_id: str = Field(description="Member user identifier")
    role: str = Field(description="Assigned role (admin, member, viewer)")
    joined_at: str = Field(description="Join timestamp (ISO 8601)")


class CreateOrgResponse(BaseModel):
    """Response after creating an organization."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Created organization UUID")
    name: str = Field(description="Organization display name")
    role: str = Field(description="Creator's assigned role ('admin')")
    created_at: str = Field(description="Creation timestamp (ISO 8601)")


class ListOrgsResponse(BaseModel):
    """List of organizations the user belongs to."""
    model_config = ConfigDict(extra="ignore")

    organizations: list[Organization] = Field(default_factory=list, description="Array of organizations")
    count: int = Field(description="Total count of organizations returned")


class OrgDetailsResponse(BaseModel):
    """Detailed organization information including member roster."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Organization UUID")
    name: str = Field(description="Organization display name")
    created_by: str = Field(description="Creator user ID")
    created_at: str = Field(description="Creation timestamp (ISO 8601)")
    members: list[OrganizationMember] = Field(default_factory=list, description="Roster of organization members")
    count: int = Field(description="Number of members")


class InviteMemberResponse(BaseModel):
    """Response upon inviting or adding a member."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    org_id: str = Field(description="Organization UUID")
    user_id: str = Field(description="Invited user ID")
    role: str = Field(description="Assigned role")


class UpdateMemberRoleResponse(BaseModel):
    """Response upon modifying a member's role."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    org_id: str = Field(description="Organization UUID")
    user_id: str = Field(description="Target user ID")
    role: str = Field(description="New assigned role")


class RemoveMemberResponse(BaseModel):
    """Response upon removing a member from an organization."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    org_id: str = Field(description="Organization UUID")
    user_id: str = Field(description="Removed user ID")


class LeaveOrgResponse(BaseModel):
    """Response upon leaving an organization."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    org_id: str = Field(description="Organization UUID")


class SelectOrgResponse(BaseModel):
    """Response upon selecting active organization context."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    token: str = Field(description="New JWT token with embedded org_id claim")
    org_id: str = Field(description="Selected organization UUID")
    role: str = Field(description="Active role in selected organization")


# ─────────────────────────────────────────────────────────────────────────────
# Security & IP Whitelisting Models
# ─────────────────────────────────────────────────────────────────────────────

class IpWhitelistEntry(BaseModel):
    """Configured IP or CIDR whitelist entry."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique whitelist entry UUID")
    user_id: str = Field(description="Owning user identifier")
    ip_or_cidr: str = Field(description="Allowed IP address or CIDR range")
    description: Optional[str] = Field(default=None, description="Human-readable description or label")
    created_at: str = Field(description="Creation timestamp (ISO 8601)")


class ListIpWhitelistResponse(BaseModel):
    """List of configured IP whitelist entries."""
    model_config = ConfigDict(extra="ignore")

    entries: list[IpWhitelistEntry] = Field(default_factory=list, description="Array of whitelist entries")
    count: int = Field(description="Total count of whitelist entries")


class DeleteIpWhitelistResponse(BaseModel):
    """Response upon deleting an IP whitelist entry."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Action status")
    message: str = Field(description="Diagnostic message")
    id: str = Field(description="UUID of deleted entry")


# ─── News Articles & Full Text Models ────────────────────────────────────────

class NewsArticleMetadata(BaseModel):
    """Metadata and preview snippet for a financial news article."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the article")
    ticker: str = Field(description="Stock ticker symbol")
    title: str = Field(description="Headline title")
    source: str = Field(description="News source / publishing agency")
    published_utc: str = Field(description="Publication timestamp in UTC")
    sentiment_score: Optional[float] = Field(default=None, description="Quantized sentiment score (-1.0 to +1.0)")
    sentiment_label: Optional[str] = Field(default=None, description="Sentiment classification label")
    confidence: Optional[float] = Field(default=None, description="Model confidence score")
    data_quality_score: Optional[float] = Field(default=None, description="Data quality score")
    snippet: str = Field(description="Short preview snippet (up to 200 characters)")


class NewsArticleFull(BaseModel):
    """Full news article body and detailed NLP metrics."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the article")
    ticker: str = Field(description="Stock ticker symbol")
    title: str = Field(description="Headline title")
    source: str = Field(description="News source / publishing agency")
    published_utc: str = Field(description="Publication timestamp in UTC")
    full_text: str = Field(description="Full text content of the article")
    sentiment_score: Optional[float] = Field(default=None, description="Quantized sentiment score (-1.0 to +1.0)")
    sentiment_label: Optional[str] = Field(default=None, description="Sentiment classification label")
    confidence: Optional[float] = Field(default=None, description="Model confidence score")
    data_quality_score: Optional[float] = Field(default=None, description="Data quality score")
    created_at: str = Field(description="Indexing timestamp in UTC")


class NewsArticlesListResponse(BaseModel):
    """Paginated list of news articles with preview snippets."""
    model_config = ConfigDict(extra="ignore")

    articles: list[NewsArticleMetadata] = Field(default_factory=list, description="Array of article metadata items")
    total: int = Field(description="Total count of articles matching filter")
    limit: int = Field(description="Page limit")
    offset: int = Field(description="Page offset")


class EntitySentimentItem(BaseModel):
    """Aggregated sentiment statistics for an extracted named entity."""
    model_config = ConfigDict(extra="ignore")

    entity_text: str = Field(description="Name or text representation of the entity (e.g. 'Apple', 'Tim Cook')")
    entity_type: str = Field(description="Categorical entity type ('company', 'person', 'product', 'location', 'organization')")
    avg_sentiment: float = Field(description="Average sentiment score (-1.0 to +1.0) across mentioning news articles")
    positive_ratio: float = Field(description="Fraction of mentions with sentiment_score > 0.1 (0.0 to 1.0)")
    negative_ratio: float = Field(description="Fraction of mentions with sentiment_score < -0.1 (0.0 to 1.0)")
    mention_count: int = Field(description="Total number of news articles mentioning this entity")
    latest_mention_date: str = Field(description="Most recent date on which this entity was mentioned (YYYY-MM-DD)")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class EntitySentimentResponse(BaseModel):
    """Response payload for Entity Sentiment Breakdown query."""
    model_config = ConfigDict(extra="ignore")

    start_date: str = Field(description="Effective start date of analysis window (YYYY-MM-DD)")
    end_date: str = Field(description="Effective end date of analysis window (YYYY-MM-DD)")
    entity_type: str = Field(description="Entity type filter applied ('all', 'company', 'person', 'product', 'location', 'organization')")
    min_mentions: int = Field(description="Minimum mentions threshold applied")
    count: int = Field(description="Number of aggregated entities returned in response")
    entities: list[EntitySentimentItem] = Field(default_factory=list, description="List of aggregated entity sentiment metrics")
    generated_at: str = Field(description="ISO 8601 UTC timestamp of response generation")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class AuditLogEntry(BaseModel):
    """An immutable compliance audit log record."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the audit log record")
    org_id: Optional[str] = Field(default=None, description="Organization UUID associated with the action")
    user_id: str = Field(description="User identifier performing the action")
    action: str = Field(description="Action code (e.g. 'apikey.create', 'org.created')")
    entity_type: str = Field(description="Target entity type (e.g. 'api_key', 'organization')")
    entity_id: Optional[str] = Field(default=None, description="Target entity ID")
    details: dict = Field(default_factory=dict, description="Structured details payload")
    ip_address: Optional[str] = Field(default=None, description="Client IP address")
    created_at: str = Field(description="UTC timestamp of the action")


class AuditLogsResponse(BaseModel):
    """Paginated response containing compliance audit log entries."""
    model_config = ConfigDict(extra="ignore")

    logs: list[AuditLogEntry] = Field(default_factory=list, description="Array of audit log entries")
    total: int = Field(description="Total count of matching records")
    limit: int = Field(description="Page limit")
    offset: int = Field(description="Page offset")


class AuditLogExportResponse(BaseModel):
    """Structured JSON payload for exported audit log reports."""
    model_config = ConfigDict(extra="ignore")

    exported_at: str = Field(description="UTC timestamp of export generation")
    total: int = Field(description="Total count of exported records")
    logs: list[AuditLogEntry] = Field(default_factory=list, description="Array of exported audit log entries")


class ApiKeyItem(BaseModel):
    """Metadata item for an API Key."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the API key")
    name: str = Field(description="Friendly name assigned to the key")
    prefix: str = Field(description="Key prefix identifier")
    created_at: str = Field(description="Creation UTC timestamp")
    revoked_at: Optional[str] = Field(default=None, description="Revocation timestamp if revoked")
    expires_at: Optional[str] = Field(default=None, description="Expiration timestamp if scheduled to expire")
    rotated_from: Optional[str] = Field(default=None, description="UUID of the previous key if rotated")
    rotation_status: str = Field(default="none", description="Rotation status ('none', 'rotating', 'rotated')")


class CreateApiKeyResponse(BaseModel):
    """Response returned upon creating a new API key."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the created API key")
    name: str = Field(description="Friendly name assigned to the key")
    api_key: str = Field(description="Full plaintext API key token (returned only once)")
    prefix: str = Field(description="Key prefix identifier")
    created_at: str = Field(description="Creation UTC timestamp")


class ListApiKeysResponse(BaseModel):
    """Response returned upon listing API keys."""
    model_config = ConfigDict(extra="ignore")

    api_keys: list[ApiKeyItem] = Field(default_factory=list, description="List of API key items")
    count: int = Field(default=0, description="Total count of returned keys")


class RotateApiKeyResponse(BaseModel):
    """Response returned upon rotating an API key."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="UUID of the newly created API key")
    name: str = Field(description="Name assigned to the new key")
    api_key: str = Field(description="Full plaintext new API key token (returned only once)")
    prefix: str = Field(description="Prefix of the new API key")
    created_at: str = Field(description="Creation UTC timestamp of new key")
    rotated_from: str = Field(description="UUID of the previous key that was rotated")
    old_key_id: str = Field(description="UUID of the previous key")
    old_key_expires_at: str = Field(description="Expiration UTC timestamp of old key")
    overlap_hours: int = Field(default=24, description="Overlap duration in hours")


class SearchResultItem(BaseModel):
    """Normalized search result item across all data domains."""
    model_config = ConfigDict(extra="ignore", populate_by_name=True)

    id: str = Field(description="Unique item identifier")
    item_type: str = Field(alias="type", description="Data domain type ('news', 'transcript', 'filing', etc.)")
    ticker: str = Field(description="Associated asset ticker symbol")
    title: str = Field(description="Descriptive title or headline")
    snippet: str = Field(description="Excerpt or preview snippet up to 200 characters")
    date: str = Field(description="Date or timestamp of the record")
    score: float = Field(description="Match relevance ranking score (0.0 to 1.0)")


class SearchResponse(BaseModel):
    """Unified Cross-Domain Search response payload."""
    model_config = ConfigDict(extra="ignore")

    query: str = Field(description="Executed search query string")
    types: list[str] = Field(default_factory=list, description="List of data domains queried")
    count: int = Field(default=0, description="Total count of results in this response page")
    results: list[SearchResultItem] = Field(default_factory=list, description="Ranked list of matching search results")
    generated_at: str = Field(description="UTC timestamp when search was executed")


class SectorRotationItem(BaseModel):
    """Individual sector rotation signal and relative strength ranking."""
    model_config = ConfigDict(extra="ignore")

    sector: str = Field(description="GICS sector display name (e.g. 'Technology', 'Financials')")
    sentiment_trend: float = Field(description="Average normalized sentiment score over lookback window")
    sentiment_momentum: float = Field(description="Sentiment momentum: recent sub-window avg minus baseline avg")
    price_momentum: float = Field(description="Average daily return of sector constituents over lookback")
    correlation_score: float = Field(description="Average pairwise Pearson correlation within sector (0.0 to 1.0)")
    relative_strength: float = Field(description="Composite relative strength score normalized to [0.0, 1.0]")
    rank: int = Field(description="Ordinal rank by relative strength (1 = strongest)")
    flag: str = Field(description="Signal flag: 'outperform', 'underperform', or 'neutral'")
    model_version: Optional[str] = Field(default=None, description="ML model version tag")
    pipeline_version: Optional[str] = Field(default=None, description="Feature extraction pipeline version")
    data_provenance: Optional[list[str]] = Field(default=None, description="Upstream data source lineage")


class SectorRotationResponse(BaseModel):
    """Response payload for Sector Rotation Signals (GET /market/sector-rotation)."""
    model_config = ConfigDict(extra="ignore")

    lookback_days: int = Field(description="Number of lookback days used for the analysis")
    include_momentum: bool = Field(description="Whether price momentum was included")
    top_n: int = Field(description="Number of sectors flagged as top/bottom performers")
    sectors: list[SectorRotationItem] = Field(default_factory=list, description="Sector items sorted by relative strength descending")
    outperform_sectors: list[str] = Field(default_factory=list, description="Sector names flagged as outperform")
    underperform_sectors: list[str] = Field(default_factory=list, description="Sector names flagged as underperform")
    market_signal: str = Field(description="Macro rotation signal: 'risk-on', 'risk-off', or 'neutral'")
    generated_at: str = Field(description="ISO-8601 UTC timestamp when analysis was generated")


# ==============================================================================
# Suite #215: Email Digest Models
# ==============================================================================

class DigestSubscription(BaseModel):
    """Stored email digest subscription configuration."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Subscription unique UUID identifier")
    user_id: str = Field(description="Owner user identifier")
    frequency: str = Field(description="Delivery cadence: 'daily' or 'weekly'")
    tickers: list[str] = Field(default_factory=list, description="Subscribed ticker symbols")
    sectors: list[str] = Field(default_factory=list, description="Subscribed GICS sector names")
    event_types: list[str] = Field(default_factory=list, description="Subscribed catalyst event categories")
    is_active: bool = Field(default=True, description="Whether subscription is actively delivering")
    created_at: str = Field(description="Creation timestamp in ISO-8601 UTC")
    updated_at: str = Field(description="Last update timestamp in ISO-8601 UTC")


class CreateDigestRequest(BaseModel):
    """Payload for creating or updating an email digest subscription."""
    model_config = ConfigDict(extra="ignore")

    frequency: str = Field(default="daily", description="Delivery frequency: 'daily' or 'weekly'")
    tickers: list[str] = Field(default_factory=list, description="List of ticker symbols to track")
    sectors: list[str] = Field(default_factory=list, description="List of GICS sectors to track")
    event_types: list[str] = Field(
        default_factory=lambda: ["earnings", "insider", "8k", "news"],
        description="Event categories: 'earnings', 'insider', 'ma', '8k', 'news', 'sentiment'",
    )
    is_active: bool = Field(default=True, description="Whether subscription is active")


class DigestSubscriptionResponse(BaseModel):
    """Response envelope for retrieving or saving an email digest subscription."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Response status indicator ('ok')")
    message: str = Field(description="Human-readable status message")
    subscription: DigestSubscription = Field(description="Active subscription configuration")


class DeleteDigestResponse(BaseModel):
    """Response envelope for deleting an email digest subscription."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Response status indicator ('deleted')")
    message: str = Field(description="Confirmation message")


class TriggerDigestRequest(BaseModel):
    """Payload for manually triggering on-demand email digest dispatch."""
    model_config = ConfigDict(extra="ignore")

    recipient_email: Optional[str] = Field(default=None, description="Override recipient email address")
    format: Optional[str] = Field(default="html", description="Preferred format: 'html' or 'text'")


class DigestItemCounts(BaseModel):
    """Summary counts of components aggregated in an email digest."""
    model_config = ConfigDict(extra="ignore")

    sentiment_count: int = Field(default=0, description="Count of ticker/sector sentiment signals")
    events_count: int = Field(default=0, description="Count of corporate catalyst events")
    news_count: int = Field(default=0, description="Count of financial news articles")


class TriggerDigestResponse(BaseModel):
    """Response envelope for triggered email digest dispatch."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Dispatch status ('sent')")
    recipient: str = Field(description="Target recipient email address")
    subject: str = Field(description="Email subject line")
    preview_html: str = Field(description="HTML formatted digest preview")
    preview_text: str = Field(description="Plain-text formatted digest preview")
    item_counts: DigestItemCounts = Field(description="Aggregated component counts")
    sent_at: str = Field(description="Dispatch timestamp in ISO-8601 UTC")


# ── Streaming Kafka Topic Access ─────────────────────────────────────────────

class KafkaTopicInfo(BaseModel):
    """Metadata describing an institutional Kafka streaming topic."""
    model_config = ConfigDict(extra="ignore")

    topic: str = Field(description="Kafka topic name (e.g. sentiment-events)")
    description: str = Field(description="High-level description of data stream")
    schema_description: str = Field(description="Schema documentation for JSON payloads")
    example_payload: Any = Field(description="Example event payload JSON object")
    partitions: int = Field(default=1, description="Number of configured topic partitions")
    retention_hours: int = Field(default=168, description="Message retention duration in hours")


class KafkaTopicsResponse(BaseModel):
    """Response payload for listing available Kafka streaming topics."""
    model_config = ConfigDict(extra="ignore")

    topics: List[KafkaTopicInfo] = Field(default_factory=list, description="Array of available streaming topics")
    total_topics: int = Field(description="Total count of available streaming topics")


class KafkaCredentials(BaseModel):
    """Temporary Kafka consumer credentials issued to the authenticated client."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique credential identifier UUID")
    user_id: str = Field(description="User identifier owning these credentials")
    username: str = Field(description="Generated SASL/SCRAM username")
    password: str = Field(description="Generated plaintext password secret")
    broker_address: str = Field(description="Kafka cluster bootstrap broker address")
    topic: str = Field(description="Scoped topic allowed for consumption")
    consumer_group: str = Field(description="Scoped consumer group identifier")
    issued_at: str = Field(description="Credential issuance timestamp (ISO-8601 UTC)")
    expires_at: str = Field(description="Credential expiration timestamp (ISO-8601 UTC)")


class RevokeKafkaCredentialsResponse(BaseModel):
    """Response payload for revoking credentials early."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Revocation status ('revoked')")
    message: str = Field(description="Confirmation message")
    id: str = Field(description="Target credential identifier UUID")
    revoked_at: str = Field(description="Revocation timestamp (ISO-8601 UTC)")


# ── Data Retention Policy Tool ──────────────────────────────────────────────

class RetentionPolicy(BaseModel):
    """Compliance data lifecycle retention policy."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique policy identifier UUID")
    org_id: Optional[str] = Field(default=None, description="Optional organization identifier if org-scoped")
    user_id: str = Field(description="User identifier owning or creating this policy")
    data_category: str = Field(description="Data category governed by this retention policy")
    retention_days: int = Field(description="Retention period duration in days (1 to 3650)")
    is_active: bool = Field(default=True, description="Whether the policy is actively enforced")
    created_at: str = Field(description="Policy creation timestamp (ISO-8601 UTC)")
    updated_at: str = Field(description="Policy last updated timestamp (ISO-8601 UTC)")


class CreateRetentionPolicyRequest(BaseModel):
    """Request payload for creating or updating a data retention policy."""
    data_category: str = Field(description="Target data category to configure")
    retention_days: int = Field(description="Retention duration in days (1 to 3650)")
    is_active: Optional[bool] = Field(default=True, description="Active status flag")


class RetentionPoliciesResponse(BaseModel):
    """Response payload for listing retention policies."""
    model_config = ConfigDict(extra="ignore")

    policies: List[RetentionPolicy] = Field(default_factory=list, description="Array of configured retention policies")
    total_policies: int = Field(description="Total count of configured retention policies")


class DeleteRetentionPolicyResponse(BaseModel):
    """Response payload for deleting/deactivating a retention policy."""
    model_config = ConfigDict(extra="ignore")

    status: str = Field(description="Deletion status ('deleted')")
    message: str = Field(description="Confirmation message")
    id: str = Field(description="Deleted policy identifier UUID")
    deleted_at: str = Field(description="Deletion timestamp (ISO-8601 UTC)")


class FactorExposureItem(BaseModel):
    """Single risk factor exposure coefficient and significance."""
    model_config = ConfigDict(extra="ignore")

    factor: str = Field(description="Risk factor name (e.g. market, momentum, sentiment, volatility)")
    beta: float = Field(description="Estimated OLS regression beta coefficient")
    t_stat: float = Field(description="Student's t-statistic for beta == 0")
    p_value: float = Field(description="Two-tailed statistical p-value")


class OLSStatistics(BaseModel):
    """Ordinary Least Squares (OLS) regression summary metrics."""
    model_config = ConfigDict(extra="ignore")

    r_squared: float = Field(description="Coefficient of determination R^2")
    adjusted_r_squared: float = Field(description="Adjusted R^2 penalized for predictor count")
    num_observations: int = Field(description="Number of observation periods in regression")
    f_statistic: float = Field(description="Overall regression F-statistic")
    p_value: float = Field(description="Statistical p-value of the regression model")


class FactorExposureResponse(BaseModel):
    """Factor exposure report response payload."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Stock ticker symbol")
    start_date: str = Field(description="Estimation window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Estimation window end date (YYYY-MM-DD)")
    benchmark_ticker: str = Field(description="Benchmark ticker used for market factor")
    factors_included: List[str] = Field(description="List of risk factors included in regression")
    ols_summary: OLSStatistics = Field(description="Model-level OLS summary statistics")
    exposures: List[FactorExposureItem] = Field(description="Individual factor exposure items")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of report generation")


class PortfolioFactorExposureRequest(BaseModel):
    """Request payload for multi-factor portfolio risk exposure and attribution."""
    model_config = ConfigDict(extra="ignore")

    tickers: List[str] = Field(description="Portfolio constituent stock ticker symbols (2 to 20 tickers)")
    weights: List[float] = Field(description="Portfolio allocation weights corresponding to tickers (must sum to 1.0 ± 0.05)")
    start_date: str = Field(description="Historical estimation window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Historical estimation window end date (YYYY-MM-DD)")
    benchmark_ticker: Optional[str] = Field(default=None, description="Benchmark ticker used for market factor (default: 'SPY')")
    factors: Optional[str] = Field(default=None, description="Comma-separated list of risk factors (default: 'market,momentum,sentiment,volatility')")


class PortfolioFactorExposureResponse(BaseModel):
    """Portfolio factor exposure and risk attribution report response payload."""
    model_config = ConfigDict(extra="ignore")

    tickers: List[str] = Field(description="Portfolio constituent ticker symbols")
    weights: List[float] = Field(description="Normalized portfolio allocation weights summing to 1.0")
    start_date: str = Field(description="Estimation window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Estimation window end date (YYYY-MM-DD)")
    benchmark_ticker: str = Field(description="Benchmark ticker used for market factor")
    factors_included: List[str] = Field(description="List of risk factors included in regression")
    ols_summary: OLSStatistics = Field(description="Model-level OLS summary statistics")
    exposures: List[FactorExposureItem] = Field(description="Portfolio-level factor exposure betas and statistical significances")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of report generation")


class ESGDimensionScore(BaseModel):
    """Quantitative metrics for a single ESG pillar dimension."""
    model_config = ConfigDict(extra="ignore")

    score: float = Field(description="Confidence-weighted average sentiment score (-1.0 to 1.0)")
    mention_count: int = Field(description="Total count of articles mentioning this pillar's keywords")
    positive_ratio: float = Field(description="Proportion of matching articles with positive sentiment (> 0.1)")
    negative_ratio: float = Field(description="Proportion of matching articles with negative sentiment (< -0.1)")


class ESGDimensions(BaseModel):
    """Composite container for the three ESG pillars."""
    model_config = ConfigDict(extra="ignore")

    environmental: ESGDimensionScore = Field(description="Environmental (E) dimension metrics")
    social: ESGDimensionScore = Field(description="Social (S) dimension metrics")
    governance: ESGDimensionScore = Field(description="Governance (G) dimension metrics")


class ESGScoresResponse(BaseModel):
    """Complete ESG sentiment scores response payload."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Stock ticker symbol, if ticker query")
    sector: Optional[str] = Field(default=None, description="GICS sector name, if sector query")
    start_date: str = Field(description="Start date for news analysis window (YYYY-MM-DD)")
    end_date: str = Field(description="End date for news analysis window (YYYY-MM-DD)")
    min_confidence: float = Field(description="Applied confidence filter threshold")
    overall_esg_score: float = Field(description="Standardized composite ESG score (0 to 100)")
    dimensions: ESGDimensions = Field(description="Pillar-by-pillar ESG dimensions breakdown")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of score calculation")


class BankruptcyComponents(BaseModel):
    """Six-pillar distress component breakdowns."""
    model_config = ConfigDict(extra="ignore")

    eight_k_distress_score: float = Field(description="SEC Form 8-K distress filing score (0.0 to 40.0 pts)")
    sentiment_deterioration_score: float = Field(description="Sentiment deterioration z-score penalty (0.0 to 20.0 pts)")
    put_call_ratio_score: float = Field(description="Options put/call ratio distress score (0.0 to 15.0 pts)")
    implied_volatility_score: float = Field(description="ATM options implied volatility distress score (0.0 to 15.0 pts)")
    supply_chain_risk_score: float = Field(description="Supply chain contagion graph risk score (0.0 to 10.0 pts)")
    insider_selling_score: float = Field(description="Executive net insider selling score (0.0 to 10.0 pts)")


class BankruptcyRiskResponse(BaseModel):
    """Complete bankruptcy risk signals response payload."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Stock ticker symbol")
    lookback_days: int = Field(description="Effective lookback observation window in calendar days")
    bankruptcy_risk_score: float = Field(description="Composite bankruptcy risk score on 0 to 100 scale")
    risk_category: str = Field(description="Risk severity classification ('LOW', 'MODERATE', 'HIGH', 'CRITICAL')")
    components: Optional[BankruptcyComponents] = Field(default=None, description="Detailed 6-pillar distress component scores")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of risk calculation")


class FXSentimentArticle(BaseModel):
    """Key driving news article for currency pair sentiment."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Article UUID identifier")
    title: str = Field(description="Article headline title")
    source: str = Field(description="Publication source or news wire")
    published_utc: str = Field(description="ISO-8601 UTC publication timestamp")
    sentiment_score: float = Field(description="Quant sentiment score (-1.0 to 1.0)")
    confidence: float = Field(description="Sentiment model confidence score (0.0 to 1.0)")
    url: str = Field(description="Source URL or news wire link")


class FXSentimentSummary(BaseModel):
    """Aggregated sentiment metrics for a currency pair."""
    model_config = ConfigDict(extra="ignore")

    avg_sentiment: float = Field(description="Confidence-weighted average sentiment score (-1.0 to 1.0)")
    mention_count: int = Field(description="Total number of relevant articles mentioning pair keywords")
    positive_ratio: float = Field(description="Proportion of articles with positive sentiment (> 0.1)")
    negative_ratio: float = Field(description="Proportion of articles with negative sentiment (< -0.1)")
    latest_article_date: Optional[str] = Field(default=None, description="Timestamp of the most recent matching article")


class FXSentimentResponse(BaseModel):
    """Complete FX Sentiment Feed response payload."""
    model_config = ConfigDict(extra="ignore")

    currency_pair: str = Field(description="Evaluated canonical currency pair symbol (e.g. 'EUR/USD')")
    start_date: str = Field(description="Start date for analyzed articles (YYYY-MM-DD)")
    end_date: str = Field(description="End date for analyzed articles (YYYY-MM-DD)")
    min_confidence: float = Field(description="Applied confidence filter threshold")
    summary: FXSentimentSummary = Field(description="Aggregated sentiment summary statistics")
    top_articles: list[FXSentimentArticle] = Field(default_factory=list, description="Top driving news articles")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of response generation")


class CommoditySentimentArticle(BaseModel):
    """Key driving news article for commodity sentiment."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Article UUID identifier")
    title: str = Field(description="Article headline title")
    source: str = Field(description="Publication source or market wire")
    published_utc: str = Field(description="ISO-8601 UTC publication timestamp")
    sentiment_score: float = Field(description="Quant sentiment score (-1.0 to 1.0)")
    confidence: float = Field(description="Sentiment model confidence score (0.0 to 1.0)")
    url: str = Field(description="Source URL or commodity wire link")


class CommoditySentimentSummary(BaseModel):
    """Aggregated sentiment metrics for a commodity asset."""
    model_config = ConfigDict(extra="ignore")

    avg_sentiment: float = Field(description="Confidence-weighted average sentiment score (-1.0 to 1.0)")
    mention_count: int = Field(description="Total number of relevant articles mentioning commodity keywords")
    positive_ratio: float = Field(description="Proportion of articles with positive sentiment (> 0.1)")
    negative_ratio: float = Field(description="Proportion of articles with negative sentiment (< -0.1)")
    latest_article_date: Optional[str] = Field(default=None, description="Timestamp of the most recent matching article")


class CommoditySentimentResponse(BaseModel):
    """Complete Commodity News Sentiment response payload."""
    model_config = ConfigDict(extra="ignore")

    commodity: str = Field(description="Evaluated canonical commodity identifier (e.g. 'crude_oil', 'gold')")
    start_date: str = Field(description="Start date for analyzed articles (YYYY-MM-DD)")
    end_date: str = Field(description="End date for analyzed articles (YYYY-MM-DD)")
    min_confidence: float = Field(description="Applied confidence filter threshold")
    summary: CommoditySentimentSummary = Field(description="Aggregated sentiment summary statistics")
    top_articles: list[CommoditySentimentArticle] = Field(default_factory=list, description="Top driving news articles")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of response generation")


class PollingWebhook(BaseModel):
    """Custom polling webhook subscription model."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique Polling Webhook identifier UUID")
    user_id: str = Field(description="Owner user identifier")
    name: str = Field(description="User-friendly subscription label")
    url: str = Field(description="Destination webhook receiver endpoint URL")
    interval_seconds: int = Field(description="Polling delivery frequency in seconds (60 to 86400)")
    query_type: str = Field(description="Data category to query: 'sentiment', 'news', 'events', or 'options'")
    query_params: dict[str, Any] = Field(default_factory=dict, description="JSON query parameters")
    secret: str = Field(description="HMAC-SHA256 secret for payload verification")
    is_active: bool = Field(default=True, description="Whether this polling subscription is active")
    last_triggered_at: Optional[str] = Field(default=None, description="Timestamp of last trigger (ISO-8601 UTC)")
    created_at: str = Field(description="Creation timestamp (ISO-8601 UTC)")


class CreatePollingWebhookRequest(BaseModel):
    """Request payload for creating a custom polling webhook."""
    model_config = ConfigDict(extra="ignore")

    name: str = Field(description="User-friendly name for this polling job")
    url: str = Field(description="Destination webhook receiver URL")
    interval_seconds: int = Field(default=300, description="Polling interval in seconds (60 to 86400)")
    query_type: str = Field(description="Query type: 'sentiment', 'news', 'events', or 'options'")
    query_params: dict[str, Any] = Field(default_factory=dict, description="Specific query filters")


class PollingWebhooksResponse(BaseModel):
    """Response envelope for listing custom polling webhooks."""
    model_config = ConfigDict(extra="ignore")

    webhooks: list[PollingWebhook] = Field(default_factory=list, description="List of user's polling webhooks")
    total: int = Field(description="Total count of polling webhooks")


class DeletePollingWebhookResponse(BaseModel):
    """Response payload upon deleting a custom polling webhook."""
    model_config = ConfigDict(extra="ignore")

    success: bool = Field(description="Whether deletion was successful")
    id: str = Field(description="Deleted webhook UUID")
    message: str = Field(description="Confirmation message")


class CryptoSentimentArticle(BaseModel):
    """Key driving news article for cryptocurrency sentiment."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Article UUID identifier")
    title: str = Field(description="Article headline title")
    source: str = Field(description="Publication source")
    published_utc: str = Field(description="Publication timestamp (ISO-8601 UTC)")
    sentiment_score: float = Field(description="Quant sentiment score (-1.0 to 1.0)")
    confidence: float = Field(description="Sentiment model confidence score (0.0 to 1.0)")
    url: str = Field(description="Source URL or institutional wire link")


class CryptoSentimentSummary(BaseModel):
    """Aggregated sentiment metrics for a cryptocurrency asset."""
    model_config = ConfigDict(extra="ignore")

    avg_sentiment: float = Field(description="Confidence-weighted average sentiment score (-1.0 to 1.0)")
    mention_count: int = Field(description="Total number of relevant articles mentioning cryptocurrency keywords")
    positive_ratio: float = Field(description="Proportion of articles with positive sentiment (> 0.1)")
    negative_ratio: float = Field(description="Proportion of articles with negative sentiment (< -0.1)")
    latest_article_date: Optional[str] = Field(default=None, description="Timestamp of the most recent matching article")


class CryptoSentimentResponse(BaseModel):
    """Complete Cryptocurrency News Sentiment response payload."""
    model_config = ConfigDict(extra="ignore")

    asset: str = Field(description="Evaluated canonical cryptocurrency symbol (e.g. 'BTC', 'ETH')")
    start_date: str = Field(description="Start date for analyzed articles (YYYY-MM-DD)")
    end_date: str = Field(description="End date for analyzed articles (YYYY-MM-DD)")
    min_confidence: float = Field(description="Applied confidence filter threshold")
    summary: CryptoSentimentSummary = Field(description="Aggregated sentiment summary statistics")
    top_articles: list[CryptoSentimentArticle] = Field(default_factory=list, description="Top driving news articles")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of response generation")


class MicrostructurePoint(BaseModel):
    """A single market microstructure observation point (VPIN and GEX)."""
    model_config = ConfigDict(extra="ignore")

    timestamp: str = Field(description="Observation timestamp (ISO-8601 UTC)")
    vpin: Optional[float] = Field(default=None, description="Volume-Synchronized Probability of Informed Trading (0.0 to 1.0)")
    gex: Optional[float] = Field(default=None, description="Net Dealer Gamma Exposure (in dollars)")


class MicrostructureResponse(BaseModel):
    """Complete Market Microstructure time series response payload."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target underlying equity ticker symbol")
    start_date: str = Field(description="Start date for analyzed window (YYYY-MM-DD)")
    end_date: str = Field(description="End date for analyzed window (YYYY-MM-DD)")
    metric: str = Field(description="Requested metric filter ('vpin', 'gex', or 'both')")
    interval: str = Field(description="Time aggregation interval ('daily' or 'intraday')")
    points: list[MicrostructurePoint] = Field(default_factory=list, description="Microstructure observation points")
    count: int = Field(description="Total count of data points returned")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of response generation")


class MarketBreadthPoint(BaseModel):
    """Daily market breadth observation point."""
    model_config = ConfigDict(extra="ignore")

    date: str = Field(description="Observation date (YYYY-MM-DD)")
    advancers: int = Field(description="Count of advancing tickers")
    decliners: int = Field(description="Count of declining tickers")
    unchanged: int = Field(description="Count of unchanged tickers")
    advance_decline_ratio: float = Field(description="Advance/Decline ratio: advancers / (advancers + decliners)")
    breadth_index: float = Field(description="Market breadth index: (advancers - decliners) / total_active (-1.0 to 1.0)")
    new_52w_highs: Optional[int] = Field(default=None, description="Count of tickers making new 52-week highs")
    new_52w_lows: Optional[int] = Field(default=None, description="Count of tickers making new 52-week lows")


class MarketBreadthResponse(BaseModel):
    """Complete Market Breadth & Advance/Decline time series response payload."""
    model_config = ConfigDict(extra="ignore")

    start_date: str = Field(description="Start date for analyzed window (YYYY-MM-DD)")
    end_date: str = Field(description="End date for analyzed window (YYYY-MM-DD)")
    universe: str = Field(description="Evaluated constituent universe ('all', 'sp500', or custom list)")
    total_tickers: int = Field(description="Total count of evaluated tickers in the universe")
    points: list[MarketBreadthPoint] = Field(default_factory=list, description="Daily market breadth observation points")
    count: int = Field(description="Total count of data points returned")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of response generation")


class ChatAlertSubscription(BaseModel):
    """Telegram or Discord chat alert subscription model."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique Subscription UUID")
    user_id: str = Field(description="Owner user identifier")
    channel_type: str = Field(description="Channel type ('telegram' or 'discord')")
    channel_target: str = Field(description="Telegram chat ID or Discord Webhook URL")
    event_types: list[str] = Field(default_factory=list, description="Subscribed event types")
    is_active: bool = Field(default=True, description="Whether subscription is active")
    created_at: str = Field(description="ISO-8601 UTC timestamp of creation")


class CreateChatAlertRequest(BaseModel):
    """Request payload for creating a chat alert subscription."""
    model_config = ConfigDict(extra="ignore")

    channel_type: str = Field(description="Channel type ('telegram' or 'discord')")
    channel_target: str = Field(description="Telegram chat ID or Discord Webhook URL")
    event_types: list[str] = Field(description="List of market event types to subscribe to")


class ChatAlertsResponse(BaseModel):
    """Response payload listing user's chat alert subscriptions."""
    model_config = ConfigDict(extra="ignore")

    subscriptions: list[ChatAlertSubscription] = Field(default_factory=list, description="Active chat alert subscriptions")
    total: int = Field(description="Total count of subscriptions")


class DeleteChatAlertResponse(BaseModel):
    """Response payload upon deleting a chat alert subscription."""
    model_config = ConfigDict(extra="ignore")

    success: bool = Field(description="Whether deletion succeeded")
    id: str = Field(description="Deleted subscription UUID")
    message: str = Field(description="Confirmation message")


class CreditSentimentResponse(BaseModel):
    """Credit default sentiment and multi-signal credit risk response payload."""
    model_config = ConfigDict(extra="ignore", populate_by_name=True)

    ticker: str = Field(description="Target stock ticker symbol")
    lookback_days: int = Field(description="Lookback observation window in calendar days")
    credit_sentiment_score: float = Field(description="Composite credit sentiment score (-1.0 to +1.0)")
    news_sentiment_avg: float = Field(description="Average sentiment score of credit-related news/disclosures")
    eight_k_distress_count: int = Field(alias="8k_distress_count", default=0, description="Count of SEC Form 8-K distress filings")
    put_call_ratio: float = Field(description="Options put/call volume ratio")
    implied_volatility: float = Field(description="At-the-money (ATM) options implied volatility")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of calculation")


class BackfillSentimentRequest(BaseModel):
    """Request payload for historical news sentiment backfill."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target stock ticker symbol (e.g. 'AAPL')")
    start_date: str = Field(description="Historical backfill start date (YYYY-MM-DD)")
    end_date: str = Field(description="Historical backfill end date (YYYY-MM-DD)")
    limit: Optional[int] = Field(default=1000, description="Maximum number of articles to process (1 to 10000)")
    overwrite: Optional[bool] = Field(default=False, description="Whether to recompute sentiment for articles with existing scores")


class BackfillSentimentResponse(BaseModel):
    """Response payload upon executing historical news sentiment backfill."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Target stock ticker symbol")
    start_date: str = Field(description="Historical backfill window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Historical backfill window end date (YYYY-MM-DD)")
    overwrite: bool = Field(description="Whether existing scores were overwritten")
    total_articles_found: int = Field(description="Total count of candidate news articles found")
    processed_articles: int = Field(description="Number of articles successfully processed with sentiment scores")
    failed_articles: int = Field(description="Number of articles that failed processing")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of backfill completion")


class PortfolioConstraints(BaseModel):
    """Constraints for portfolio asset weight optimization."""
    model_config = ConfigDict(extra="ignore")

    long_only: Optional[bool] = Field(default=True, description="Enforce long-only portfolio weights (w_i >= 0, sum(w) = 1.0)")


class PortfolioWeight(BaseModel):
    """Individual constituent asset weight allocation."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Asset ticker symbol")
    weight: float = Field(description="Optimized portfolio weight allocation (0.0 to 1.0)")


class PortfolioOptimizeRequest(BaseModel):
    """Request payload for portfolio asset weight optimization."""
    model_config = ConfigDict(extra="ignore")

    tickers: List[str] = Field(description="Universe of asset ticker symbols (2 to 20 constituents)")
    start_date: str = Field(description="Historical analysis window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Historical analysis window end date (YYYY-MM-DD)")
    optimization_type: Optional[str] = Field(default="max_sharpe", description="Optimization method ('max_sharpe' or 'risk_parity')")
    risk_free_rate: Optional[float] = Field(default=0.0, description="Annualized risk-free rate (0.0 to 0.10)")
    constraints: Optional[PortfolioConstraints] = Field(default=None, description="Optional portfolio construction constraints")


class PortfolioOptimizeResponse(BaseModel):
    """Response payload upon portfolio optimization computation."""
    model_config = ConfigDict(extra="ignore")

    tickers: List[str] = Field(description="Ordered universe of constituent stock tickers")
    start_date: str = Field(description="Historical analysis window start date (YYYY-MM-DD)")
    end_date: str = Field(description="Historical analysis window end date (YYYY-MM-DD)")
    optimization_type: str = Field(description="Optimization objective applied ('max_sharpe' or 'risk_parity')")
    risk_free_rate: float = Field(description="Annualized risk-free rate applied")
    weights: List[PortfolioWeight] = Field(description="Optimal asset weight allocations summing to 1.0")
    expected_annual_return: float = Field(description="Expected annualized portfolio return")
    expected_annual_volatility: float = Field(description="Expected annualized portfolio volatility")
    sharpe_ratio: float = Field(description="Annualized Sharpe Ratio")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of calculation")


class RetrainingJob(BaseModel):
    """Model retraining job record."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique identifier of the retraining job (UUID)")
    org_id: Optional[str] = Field(default=None, description="Optional organization identifier associated with the job")
    user_id: str = Field(description="User identifier who created or triggered the job")
    model_type: str = Field(default="sentiment", description="Target model type (e.g. sentiment, finbert, minilm)")
    trigger_type: str = Field(default="manual", description="Trigger classification ('manual' or 'scheduled')")
    status: str = Field(description="Current job status ('pending', 'running', 'completed', 'failed', 'cancelled')")
    config: dict[str, Any] = Field(default_factory=dict, description="Hyperparameter and dataset configuration payload")
    created_at: str = Field(description="ISO-8601 UTC timestamp of job creation")
    started_at: Optional[str] = Field(default=None, description="ISO-8601 UTC timestamp when training began")
    completed_at: Optional[str] = Field(default=None, description="ISO-8601 UTC timestamp when training completed, failed, or was cancelled")
    metrics: Optional[dict[str, Any]] = Field(default=None, description="Evaluation and training performance metrics")
    error_message: Optional[str] = Field(default=None, description="Error message description if job failed or cancellation reason")


class CreateRetrainingJobRequest(BaseModel):
    """Request payload for creating a new model retraining job."""
    model_config = ConfigDict(extra="ignore")

    model_type: Optional[str] = Field(default="sentiment", description="Target model type (default: sentiment)")
    trigger_type: Optional[str] = Field(default="manual", description="Trigger type ('manual' or 'scheduled', default: manual)")
    config: Optional[dict[str, Any]] = Field(default=None, description="Hyperparameter training configuration")


class RetrainingJobResponse(BaseModel):
    """Single retraining job response envelope."""
    model_config = ConfigDict(extra="ignore")

    job: RetrainingJob = Field(description="The retraining job entity")
    message: str = Field(description="Status description or action confirmation message")


class ListRetrainingJobsResponse(BaseModel):
    """Paginated list response for model retraining jobs."""
    model_config = ConfigDict(extra="ignore")

    jobs: list[RetrainingJob] = Field(default_factory=list, description="List of retraining job records")
    total: int = Field(description="Total count of matching retraining jobs across all pages")
    limit: int = Field(description="Page size limit")
    offset: int = Field(description="Page offset")


class FIXOrderRequest(BaseModel):
    """Payload for submitting a FIX NewOrderSingle (35=D) message."""
    fix_message: str = Field(description="Raw pipe-delimited or SOH-delimited FIX 4.4 order message string")


class FIXCancelRequest(BaseModel):
    """Payload for submitting a FIX OrderCancelRequest (35=F) message."""
    fix_message: str = Field(description="Raw pipe-delimited or SOH-delimited FIX 4.4 cancel message string")


class FIXOrderResponse(BaseModel):
    """Execution report response payload for FIX orders and cancel requests."""
    model_config = ConfigDict(extra="ignore")

    order_id: str = Field(description="Server-assigned unique Order ID (UUID) (Tag 37)")
    exec_id: str = Field(description="Server-assigned unique Execution ID (UUID) (Tag 17)")
    cl_ord_id: str = Field(description="Client-specified unique order identifier (Tag 11)")
    symbol: str = Field(description="Financial asset symbol (Tag 55)")
    side: str = Field(description="Order side: '1' (Buy) or '2' (Sell) (Tag 54)")
    order_type: str = Field(description="Order type: '1' (Market) or '2' (Limit) (Tag 40)")
    qty: float = Field(description="Total requested order quantity (Tag 38)")
    filled_qty: float = Field(description="Cumulative executed/filled quantity (Tag 14)")
    avg_price: Optional[float] = Field(default=None, description="Volume-weighted average executed price (Tag 6)")
    status: str = Field(description="Order lifecycle status: 'open', 'filled', 'cancelled', or 'rejected'")
    exec_type: str = Field(description="FIX Execution Type: '0' (New), '1' (Partial Fill), '2' (Fill), '4' (Cancelled), '8' (Rejected) (Tag 150)")
    fix_message: str = Field(description="Formatted FIX Execution Report (35=8) or Reject protocol string")
    created_at: str = Field(description="ISO-8601 timestamp of order creation or execution report generation")


class FixOrderItem(BaseModel):
    """Summary item representing an order in order list queries."""
    model_config = ConfigDict(extra="ignore")

    order_id: str = Field(description="Server-assigned unique Order ID (UUID)")
    cl_ord_id: str = Field(description="Client-specified order identifier")
    symbol: str = Field(description="Financial asset symbol")
    side: str = Field(description="Order side: '1' (Buy) or '2' (Sell)")
    order_type: str = Field(description="Order type: '1' (Market) or '2' (Limit)")
    qty: float = Field(description="Total requested quantity")
    filled_qty: float = Field(description="Cumulative executed quantity")
    avg_price: Optional[float] = Field(default=None, description="Volume-weighted average price (if executed)")
    status: str = Field(description="Order lifecycle status: 'open', 'filled', 'cancelled', or 'rejected'")
    created_at: str = Field(description="ISO-8601 order creation timestamp")


FIXOrderItem = FixOrderItem


class FIXOrdersListResponse(BaseModel):
    """Paginated list of user FIX orders."""
    model_config = ConfigDict(extra="ignore")

    orders: list[FixOrderItem] = Field(default_factory=list, description="Array of matching order summaries")
    total: int = Field(description="Total count of matching orders before pagination")
    limit: int = Field(description="Effective limit parameter applied")
    offset: int = Field(description="Effective offset parameter applied")


class DLQEventItem(BaseModel):
    """Summary item representing a failed event in DLQ list queries."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Internal unique identifier UUID for the DLQ record")
    event_id: str = Field(description="Upstream source event identifier string")
    source: str = Field(description="Originating subsystem/pipeline source (e.g. 'sentiment', 'ingestion')")
    error_type: Optional[str] = Field(default=None, description="Error classification category")
    error_message: Optional[str] = Field(default=None, description="Narrative description of failure reason")
    payload_preview: str = Field(description="Truncated preview snippet of the event payload")
    failed_at: str = Field(description="Timestamp when failure occurred in ISO-8601 format")
    retry_count: int = Field(description="Total number of automated or manual retry attempts")
    status: str = Field(description="Lifecycle status: 'failed', 'retrying', 'reprocessed', 'purged'")
    created_at: str = Field(description="Creation timestamp in ISO-8601 format")
    updated_at: str = Field(description="Last update timestamp in ISO-8601 format")


class DLQEventDetail(BaseModel):
    """Comprehensive details of a specific DLQ event, including full un-truncated payload."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Internal unique identifier UUID for the DLQ record")
    event_id: str = Field(description="Upstream source event identifier string")
    source: str = Field(description="Originating subsystem/pipeline source")
    error_type: Optional[str] = Field(default=None, description="Error classification category")
    error_message: Optional[str] = Field(default=None, description="Full narrative description of failure reason")
    payload: Any = Field(description="Full original message payload JSON structure")
    failed_at: str = Field(description="Timestamp when failure occurred in ISO-8601 format")
    retry_count: int = Field(description="Total number of automated or manual retry attempts")
    status: str = Field(description="Lifecycle status: 'failed', 'retrying', 'reprocessed', 'purged'")
    created_at: str = Field(description="Creation timestamp in ISO-8601 format")
    updated_at: str = Field(description="Last update timestamp in ISO-8601 format")


class DLQEventsListResponse(BaseModel):
    """Paginated list response envelope for DLQ events query."""
    model_config = ConfigDict(extra="ignore")

    events: list[DLQEventItem] = Field(default_factory=list, description="Array of matching DLQ event summaries")
    total: int = Field(description="Total count of matching records across all pages")
    limit: int = Field(description="Maximum limit of items returned in current page")
    offset: int = Field(description="Pagination offset applied")


class ReprocessDLQResponse(BaseModel):
    """Response payload confirming immediate reprocessing initiation for a DLQ event."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique DLQ record identifier UUID")
    event_id: str = Field(description="Upstream source event identifier string")
    status: str = Field(description="Updated lifecycle status after reprocessing ('reprocessed' or 'retrying')")
    retry_count: int = Field(description="Updated total retry count")
    message: str = Field(description="Operational status message")
    reprocessed_at: str = Field(description="Reprocessing timestamp in ISO-8601 format")


class PurgeDLQResponse(BaseModel):
    """Response payload confirming permanent purge of a DLQ event."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique DLQ record identifier UUID")
    event_id: str = Field(description="Upstream source event identifier string")
    status: str = Field(description="Final purged lifecycle status ('purged')")
    message: str = Field(description="Confirmation message")
    purged_at: str = Field(description="Purge timestamp in ISO-8601 format")


class SLAStatusResponse(BaseModel):
    """Response payload containing API latency percentiles and contractual SLA compliance status."""
    model_config = ConfigDict(extra="ignore")

    start_date: str = Field(description="Start date for the aggregated window in YYYY-MM-DD format")
    end_date: str = Field(description="End date for the aggregated window in YYYY-MM-DD format")
    total_requests: int = Field(description="Total number of requests processed in the specified window")
    average_latency_ms: float = Field(description="Volume-weighted arithmetic mean latency in milliseconds")
    percentiles: dict[str, float] = Field(default_factory=dict, description="Latency percentiles computed via linear interpolation (e.g. {'p50': 4.8, 'p95': 14.2})")
    sla_target_ms: int = Field(description="Latency SLA target threshold in milliseconds")
    compliant_requests: int = Field(description="Count of requests executed with latency <= sla_target_ms")
    sla_compliance_rate: float = Field(description="SLA compliance percentage ((compliant_requests / total_requests) * 100.0)")
    sla_status: str = Field(description="Overall SLA compliance status: 'met' if sla_compliance_rate >= threshold, else 'breached'")
    generated_at: str = Field(description="ISO-8601 timestamp when this SLA compliance report was generated")


class StageBreakdown(BaseModel):
    """Average latency breakdown across signal pipeline stages in milliseconds."""
    model_config = ConfigDict(extra="ignore")

    fetch_latency_ms: float = Field(description="Average latency to fetch raw document via HTTP or WebSocket in milliseconds")
    normalization_latency_ms: float = Field(description="Average in-process preprocessing runtime in milliseconds")
    inference_latency_ms: float = Field(description="Average ONNX sentiment inference runtime in milliseconds")
    write_latency_ms: float = Field(description="Average database ILP write / buffer commit runtime in milliseconds")


class SLALatencyResponse(BaseModel):
    """Signal Ingestion and Processing Pipeline Latency SLA Compliance Report."""
    model_config = ConfigDict(extra="ignore")

    ticker: Optional[str] = Field(default=None, description="Filtered ticker symbol or None for universe-wide aggregation")
    start_date: str = Field(description="Start date for the aggregated window in YYYY-MM-DD format")
    end_date: str = Field(description="End date for the aggregated window in YYYY-MM-DD format")
    total_signals: int = Field(description="Total number of processed signals analyzed in the window")
    average_latency_ms: float = Field(description="Volume-weighted arithmetic mean total signal freshness latency in milliseconds")
    percentiles: dict[str, float] = Field(default_factory=dict, description="Total signal latency percentiles (e.g. {'p50': 35.2, 'p95': 88.4})")
    max_latency_ms: float = Field(description="Maximum signal latency recorded in the window in milliseconds")
    sla_target_ms: int = Field(description="Target SLA latency threshold in milliseconds")
    sla_compliant_signals: int = Field(description="Count of signals with total latency <= sla_target_ms")
    sla_compliance_rate: float = Field(description="SLA compliance percentage")
    sla_status: str = Field(description="Overall SLA compliance status: 'met' or 'breached'")
    stage_breakdown: StageBreakdown = Field(description="Average latency breakdown across pipeline stages")
    generated_at: str = Field(description="ISO-8601 timestamp when this report was generated")


class SandboxStatusResponse(BaseModel):
    """Response payload representing the active status and metadata of the API Sandbox environment."""
    model_config = ConfigDict(extra="ignore")

    active: bool = Field(description="Whether the sandbox environment is currently active for the authenticated user")
    mock_data_version: str = Field(default="sandbox-v1.0", description="Identifier string for the simulated mock data catalog and versioning")
    activated_at: Optional[str] = Field(default=None, description="Timestamp when sandbox mode was activated (if active)")
    deactivated_at: Optional[str] = Field(default=None, description="Timestamp when sandbox mode was deactivated (if inactive)")
    available_endpoints: list[str] = Field(default_factory=list, description="Key API endpoints available with isolated mock simulation")
    message: str = Field(description="Operational status message")


# Ergonomic alias
SandboxStatus = SandboxStatusResponse


class ProcessingStep(BaseModel):
    """A granular processing execution step within a record's data provenance trace."""
    model_config = ConfigDict(extra="ignore")

    step: str = Field(description="Step stage identifier (e.g. 'fetch_article', 'clean_text', 'infer_sentiment')")
    timestamp: str = Field(description="ISO 8601 UTC timestamp when the processing step was performed")
    description: str = Field(description="Human-readable description of what this processing step achieved")
    details: Optional[dict[str, Any]] = Field(default=None, description="Optional structured details and execution metrics")


class DataProvenanceItem(BaseModel):
    """An immutable data provenance and lineage audit record."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique provenance record UUID identifier")
    record_type: str = Field(description="Record entity type ('sentiment' or 'news')")
    record_id: str = Field(description="Unique identifier of the target record")
    source_type: str = Field(description="Source origin provider ('sec_edgar', 'finnhub', 'polygon', 'manual', 'mock')")
    source_id: Optional[str] = Field(default=None, description="Upstream source article ID or filing accession number")
    model_version: str = Field(description="ML / Transformer sentiment model version tag")
    pipeline_version: str = Field(description="Feature extraction & preprocessing pipeline version")
    data_quality_score: Optional[float] = Field(default=None, description="Quantitative data quality score (0.0 to 1.0)")
    processing_steps: list[ProcessingStep] = Field(default_factory=list, description="Chronological execution steps comprising the processing history")
    created_at: str = Field(description="ISO 8601 UTC timestamp when the provenance entry was recorded")


class DataProvenanceResponse(BaseModel):
    """Response envelope for data provenance lineage trace query."""
    model_config = ConfigDict(extra="ignore")

    record_type: str = Field(description="Queried record type ('sentiment' or 'news')")
    record_id: str = Field(description="Queried record identifier")
    provenance_entries: list[DataProvenanceItem] = Field(default_factory=list, description="List of matching provenance audit entries")
    total_entries: int = Field(description="Total count of matching provenance records")
    retrieved_at: str = Field(description="ISO 8601 UTC timestamp of retrieval query")


DataProvenance = DataProvenanceResponse


class SentimentAnomalyAlert(BaseModel):
    """Real-time statistical sentiment anomaly alert frame pushed over WebSocket."""
    model_config = ConfigDict(extra="ignore")

    type: str = Field(default="sentiment_anomaly", description="Event type identifier ('sentiment_anomaly')")
    ticker: str = Field(description="Target equity ticker symbol (e.g. AAPL, NVDA)")
    latest_score: float = Field(description="Most recently observed sentiment score [-1.0, +1.0]")
    mean_score: float = Field(description="Historical rolling sample mean baseline")
    stddev: float = Field(description="Historical rolling sample standard deviation")
    zscore: float = Field(description="Statistical z-score deviation")
    direction: str = Field(description="Anomaly deviation direction ('bullish' or 'bearish')")
    timestamp: str = Field(description="ISO 8601 UTC timestamp of latest observation")


class AnomalyScanResponse(BaseModel):
    """Response envelope for on-demand sentiment anomaly scan trigger."""
    model_config = ConfigDict(extra="ignore")

    anomalies_found: int = Field(description="Total number of anomalies detected in this scan")
    alerts_broadcasted: int = Field(description="Total number of alerts broadcast to active WebSocket subscribers")
    anomalies: list[SentimentAnomalyAlert] = Field(default_factory=list, description="List of detected anomaly alerts")
    scanned_at: str = Field(description="ISO 8601 UTC timestamp when scan completed")
    message: str = Field(description="Diagnostic execution summary")


SentimentAnomalyNotification = SentimentAnomalyAlert


class LanguageDetectionResponse(BaseModel):
    """Language detection and multilingual model routing response."""
    model_config = ConfigDict(extra="ignore")

    language: str = Field(description="Detected language name (e.g. 'english', 'japanese', 'spanish')")
    language_code: str = Field(description="ISO 639-1 two-letter language code (e.g. 'en', 'ja', 'es')")
    confidence: float = Field(description="Detection confidence score (0.0 to 1.0)")
    analyzed_chars: int = Field(description="Number of characters analyzed from the input text")
    is_multilingual_model_applied: bool = Field(description="Whether the multilingual inference model was selected for routing")
    model_version: str = Field(description="Assigned inference model version string (e.g. 'finbert-v3.1.0', 'multilingual-minilm-v1.0')")
    message: str = Field(description="Diagnostic or operational message")


class HardwareRequirements(BaseModel):
    """Hardware infrastructure specifications for optimal model execution."""
    model_config = ConfigDict(extra="ignore")

    cpu: str = Field(description="Recommended vCPU count for concurrent thread pooling")
    memory_gb: int = Field(description="Minimum system memory in gigabytes")
    gpu: str = Field(description="Optional GPU acceleration tier")


class VersionHistoryItem(BaseModel):
    """Release version and change log entry for model lineage tracking."""
    model_config = ConfigDict(extra="ignore")

    version: str = Field(description="Semantic version of the model artifact")
    release_date: str = Field(description="Release date in YYYY-MM-DD format")
    changes: str = Field(description="Architectural or training delta summary")


class LicensingInfo(BaseModel):
    """Model licensing and proprietary training corpus rights information."""
    model_config = ConfigDict(extra="ignore")

    model_license: str = Field(description="Intellectual property and usage license terms")
    training_data_rights: str = Field(description="Data rights verification status")


class ModelCardResponse(BaseModel):
    """Standardized Model Card and Lineage Governance Report."""
    model_config = ConfigDict(extra="ignore")

    model_id: str = Field(description="Canonical model identifier")
    model_name: str = Field(description="Human-readable model title and task description")
    architecture: str = Field(description="Neural network architecture family")
    base_model: str = Field(description="Upstream foundation model checkpoint")
    fine_tuning_dataset: str = Field(description="Dataset used for domain-specific fine-tuning")
    task: str = Field(description="Downstream quantitative NLP task")
    precision: str = Field(description="Floating-point inference precision")
    quantization: str = Field(description="Model weight quantization strategy")
    sequence_length: int = Field(description="Maximum sequence length / context window in tokens")
    chunking_strategy: str = Field(description="Document chunking and sliding window strategy")
    mean_latency_ms: float = Field(description="Arithmetic mean inference latency in milliseconds")
    p95_latency_ms: float = Field(description="95th percentile inference latency in milliseconds")
    p99_latency_ms: float = Field(description="99th percentile inference latency in milliseconds")
    hardware_requirements: HardwareRequirements = Field(description="Minimum and recommended hardware infrastructure")
    version_history: list[VersionHistoryItem] = Field(default_factory=list, description="Chronological list of model versions and evolution")
    licensing: LicensingInfo = Field(description="Licensing and data rights compliance terms")


# Ergonomic alias
ModelCard = ModelCardResponse


# ═══════════════════════════════════════════════════════════════════════════════
# Alpha Signal Validation Report Models
# ═══════════════════════════════════════════════════════════════════════════════

class AlphaSignalConfig(BaseModel):
    """Strategy Signal Generation Parameters."""
    model_config = ConfigDict(extra="ignore")

    signal_type: str = Field(default="sentiment", description="Signal type indicator, e.g. 'sentiment', 'sentiment_anomaly', 'esg'")
    threshold_long: float = Field(default=0.2, description="Sentiment score threshold to enter a Long position")
    threshold_short: float = Field(default=-0.2, description="Sentiment score threshold to enter a Short position")
    holding_days: int = Field(default=5, description="Minimum holding horizon in calendar days")
    smoothing_window_days: Optional[int] = Field(default=None, description="Optional rolling moving average smoothing window in days")


class AlphaReportRequest(BaseModel):
    """Alpha Strategy Validation Backtest Request."""
    model_config = ConfigDict(extra="ignore")

    tickers: list[str] = Field(description="Target constituent ticker symbols (1 to 10 tickers)")
    start_date: str = Field(description="Start date for historical evaluation in YYYY-MM-DD format")
    end_date: str = Field(description="End date for historical evaluation in YYYY-MM-DD format")
    signal_config: AlphaSignalConfig = Field(description="Signal generation and threshold configuration")
    benchmark_ticker: Optional[str] = Field(default="SPY", description="Benchmark asset ticker for relative performance attribution")
    initial_capital: Optional[float] = Field(default=1_000_000.0, description="Initial portfolio capital balance in USD")


class PerformanceMetrics(BaseModel):
    """Comprehensive Institutional Performance and Risk Attribution Metrics."""
    model_config = ConfigDict(extra="ignore")

    total_return: float = Field(description="Total cumulative strategy return over the backtest period")
    annualized_return: float = Field(description="Compound annualized strategy return (252 trading days)")
    annualized_volatility: float = Field(description="Annualized strategy standard deviation / volatility")
    sharpe_ratio: float = Field(description="Annualized Sharpe ratio (risk-free rate = 0.0)")
    sortino_ratio: float = Field(description="Annualized Sortino ratio (downside risk deviation)")
    max_drawdown: float = Field(description="Maximum peak-to-trough portfolio equity drawdown")
    win_rate: float = Field(description="Percentage of closed round-trip trades with positive net returns")
    profit_factor: float = Field(description="Ratio of gross winning profits to gross losing losses")
    total_trades: int = Field(description="Total number of position transitions / trades executed")
    avg_holding_period_days: float = Field(description="Average holding period per trade in calendar days")
    benchmark_ticker: str = Field(description="Benchmark ticker symbol used for relative comparison")
    benchmark_total_return: float = Field(description="Total cumulative benchmark return over the period")
    benchmark_annualized_return: float = Field(description="Compound annualized benchmark return")
    benchmark_annualized_volatility: float = Field(description="Annualized benchmark standard deviation / volatility")
    alpha: float = Field(description="Net excess alpha return over benchmark")
    beta: float = Field(description="Systematic market sensitivity beta from OLS regression")
    information_ratio: float = Field(description="Annualized Active Return / Annualized Tracking Error")
    tracking_error: float = Field(description="Annualized standard deviation of excess daily returns")


class EquityCurvePoint(BaseModel):
    """Point-in-Time Daily Equity and Position State."""
    model_config = ConfigDict(extra="ignore")

    date: str = Field(description="Date of observation in YYYY-MM-DD format")
    portfolio_value: float = Field(description="Total mark-to-market strategy portfolio equity value in USD")
    benchmark_value: float = Field(description="Mark-to-market benchmark equity value in USD")
    strategy_daily_return: float = Field(description="Net daily portfolio strategy return")
    benchmark_daily_return: float = Field(description="Daily benchmark asset return")
    position: int = Field(description="Aggregated portfolio position state (-1 = Short, 0 = Flat, 1 = Long)")


class AlphaReportResponse(BaseModel):
    """Response payload containing complete Alpha Validation Report, Risk Metrics, and Equity Curves."""
    model_config = ConfigDict(extra="ignore")

    tickers: list[str] = Field(description="Portfolio constituent ticker symbols analyzed")
    start_date: str = Field(description="Start date of backtest evaluation (YYYY-MM-DD)")
    end_date: str = Field(description="End date of backtest evaluation (YYYY-MM-DD)")
    signal_config: AlphaSignalConfig = Field(description="Evaluated signal configuration")
    initial_capital: float = Field(description="Starting capital allocation")
    metrics: PerformanceMetrics = Field(description="Summary risk and return performance metrics")
    equity_curve: list[EquityCurvePoint] = Field(default_factory=list, description="Complete daily equity curve and active position timeseries")
    generated_at: str = Field(description="ISO-8601 UTC timestamp when this report was compiled")
    message: str = Field(description="Operational diagnostic and data source attribution message")


# Ergonomic alias
AlphaReport = AlphaReportResponse


# ═══════════════════════════════════════════════════════════════════════════════
# Point-in-Time (PIT) Replay Models
# ═══════════════════════════════════════════════════════════════════════════════

class PITReplayNewsItem(BaseModel):
    """Point-in-Time News Article Snapshot."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique article identifier")
    title: str = Field(description="Headline title")
    source: str = Field(description="Originating news source or agency")
    published_utc: str = Field(description="Source event publication timestamp (ISO-8601 UTC)")
    ingested_utc: str = Field(description="Platform ingestion timestamp (ISO-8601 UTC)")
    db_commit_utc: str = Field(description="Database commit timestamp (ISO-8601 UTC)")
    sentiment_score: float = Field(description="Associated sentiment score (-1.0 to 1.0)")


class PITReplayFilingItem(BaseModel):
    """Point-in-Time Regulatory Filing Snapshot."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Filing unique record identifier")
    form_type: str = Field(description="SEC Form type (e.g. '8-K', '10-Q', '10-K')")
    filing_date: str = Field(description="SEC filing date (YYYY-MM-DD)")
    accession_number: str = Field(description="SEC EDGAR accession number")
    event_category: str = Field(description="Material event category classification")
    published_utc: str = Field(description="Public release timestamp (ISO-8601 UTC)")
    ingested_utc: str = Field(description="System ingestion timestamp (ISO-8601 UTC)")


class PITReplayEventItem(BaseModel):
    """Point-in-Time Corporate/Market Event Snapshot."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Event unique record identifier")
    event_type: str = Field(description="Material corporate event type")
    ticker: str = Field(description="Target stock ticker symbol")
    event_date: str = Field(description="Corporate event occurrence date (YYYY-MM-DD)")
    published_utc: str = Field(description="Event announcement timestamp (ISO-8601 UTC)")
    ingested_utc: str = Field(description="Ingestion timestamp (ISO-8601 UTC)")


class PITReplaySentimentItem(BaseModel):
    """Point-in-Time Sentiment Record Snapshot."""
    model_config = ConfigDict(extra="ignore")

    id: str = Field(description="Unique sentiment scoring record identifier")
    ticker: str = Field(description="Target stock ticker symbol")
    published_utc: str = Field(description="Source event publication timestamp (ISO-8601 UTC)")
    ingested_utc: str = Field(description="Platform ingestion timestamp (ISO-8601 UTC)")
    db_commit_utc: str = Field(description="Database commit timestamp (ISO-8601 UTC)")
    sentiment_score: float = Field(description="Quantized sentiment score (-1.0 to 1.0)")
    sentiment_label: str = Field(description="Sentiment classification label")
    confidence: float = Field(description="Classification prediction confidence score (0.0 to 1.0)")
    valid_from: Optional[str] = Field(default=None, description="Point-in-time validity start timestamp in ISO-8601 UTC (SCD Type 2)")
    valid_to: Optional[str] = Field(default=None, description="Point-in-time validity end timestamp in ISO-8601 UTC (None if current version) (SCD Type 2)")
    revision_number: Optional[int] = Field(default=None, description="Slowly Changing Dimension revision number (starts at 1)")
    is_current: Optional[bool] = Field(default=None, description="True if this record represents the latest active revision")


class PITReplaySummary(BaseModel):
    """Point-in-Time Category Record Counts and Summary."""
    model_config = ConfigDict(extra="ignore")

    news_count: int = Field(description="Number of point-in-time news articles returned")
    filings_count: int = Field(description="Number of point-in-time regulatory filings returned")
    events_count: int = Field(description="Number of point-in-time corporate events returned")
    sentiment_count: int = Field(description="Number of point-in-time sentiment records returned")
    total_records: int = Field(description="Total count of all point-in-time records across categories")


class PITReplayConsistency(BaseModel):
    """Point-in-Time Temporal Consistency and Look-Ahead-Bias Verification."""
    model_config = ConfigDict(extra="ignore")

    all_records_consistent: bool = Field(description="Whether all returned records strictly satisfy published_utc <= ingested_utc <= db_commit_utc <= as_of_utc")
    violations_count: int = Field(description="Number of temporal order or look-ahead violations detected")
    invariant: str = Field(description="Strict temporal invariant evaluated")
    message: str = Field(description="Consistency evaluation summary message")


class PITReplayResponse(BaseModel):
    """Response payload containing reconstructed historical state and look-ahead audit validation."""
    model_config = ConfigDict(extra="ignore")

    ticker: str = Field(description="Requested target stock ticker symbol")
    as_of_utc: str = Field(description="Historical as-of point-in-time timestamp evaluated")
    news_articles: list[PITReplayNewsItem] = Field(default_factory=list, description="News articles visible to the model as of the timestamp")
    filings: list[PITReplayFilingItem] = Field(default_factory=list, description="Regulatory filings visible to the model as of the timestamp")
    events: list[PITReplayEventItem] = Field(default_factory=list, description="Corporate and market events visible to the model as of the timestamp")
    sentiment_records: list[PITReplaySentimentItem] = Field(default_factory=list, description="Sentiment scoring records visible to the model as of the timestamp")
    summary: PITReplaySummary = Field(description="Category and total record counts summary")
    replay_consistency: PITReplayConsistency = Field(description="Look-ahead-bias and temporal consistency verification report")
    model_generated_at: str = Field(description="Maximum db_commit_utc timestamp across all visible records")
    message: str = Field(description="Diagnostic and status message")


# Ergonomic alias
PITReplay = PITReplayResponse


# ═══════════════════════════════════════════════════════════════════════════════
# Signal Quality Report & Predictive Power Models
# ═══════════════════════════════════════════════════════════════════════════════

class SignalQualityReportRequest(BaseModel):
    """Request payload for evaluating historical Signal Quality and Predictive Power."""
    model_config = ConfigDict(extra="ignore")

    signal_type: str = Field(description="Signal family stream to evaluate ('sentiment', 'spillover', 'gex', 'insider', 'event')")
    tickers: list[str] = Field(description="Constituent stock ticker symbols (1 to 20 tickers)")
    start_date: str = Field(description="Start date for historical evaluation in YYYY-MM-DD format")
    end_date: str = Field(description="End date for historical evaluation in YYYY-MM-DD format")
    horizon_days: Optional[int] = Field(default=5, description="Forward return holding horizon in trading days (1 to 20)")
    benchmark_ticker: Optional[str] = Field(default="SPY", description="Benchmark asset ticker for relative comparison")


class ICSummary(BaseModel):
    """Information Coefficient (IC) Statistical Summary."""
    model_config = ConfigDict(extra="ignore")

    spearman_ic: float = Field(description="Spearman Rank correlation between signal value and forward asset returns")
    rank_ic: float = Field(description="Normalized Rank IC across cross-sectional observations")
    observations: int = Field(description="Total count of valid paired signal and return observations")
    icir: Optional[float] = Field(default=None, description="Information Coefficient Information Ratio (mean IC / sample stddev of ICs across evaluation periods)")


class DecayCurvePoint(BaseModel):
    """Point on the Signal Horizon Decay Curve."""
    model_config = ConfigDict(extra="ignore")

    horizon_days: int = Field(description="Forward return horizon in trading days")
    ic: float = Field(description="Estimated Information Coefficient (IC) at this forward horizon")


class MarketCapBias(BaseModel):
    """Market Capitalization Quantile Bias Analytics."""
    model_config = ConfigDict(extra="ignore")

    top_quantile_avg: float = Field(description="Average signal strength across top market-cap quartile (Large Cap)")
    bottom_quantile_avg: float = Field(description="Average signal strength across bottom market-cap quartile (Small Cap)")
    spread: float = Field(description="Spread between bottom and top quartile averages")


class SignalQualityReportResponse(BaseModel):
    """Comprehensive Signal Quality & Predictive Validity Report."""
    model_config = ConfigDict(extra="ignore")

    signal_type: str = Field(description="Evaluated signal family stream")
    tickers: list[str] = Field(description="Evaluated stock constituent ticker symbols")
    start_date: str = Field(description="Backtest evaluation start date (YYYY-MM-DD)")
    end_date: str = Field(description="Backtest evaluation end date (YYYY-MM-DD)")
    horizon_days: int = Field(description="Forward return holding horizon evaluated")
    coverage_pct: float = Field(description="Fraction of evaluation days with active signal generation per ticker")
    freshness_avg_ms: float = Field(description="Average signal ingestion latency in milliseconds")
    freshness_p95_ms: float = Field(description="95th percentile signal ingestion latency in milliseconds")
    ic_summary: ICSummary = Field(description="Information Coefficient (IC) and Rank IC statistical metrics")
    decay_curve: list[DecayCurvePoint] = Field(default_factory=list, description="Information coefficient decay trajectory")
    half_life_days: float = Field(description="Estimated exponential signal half-life in trading days")
    hit_rate_pct: float = Field(description="Percentage of observations where signal direction matched forward return direction")
    false_positive_rate_pct: float = Field(description="Percentage of positive signals that resulted in negative forward returns")
    sector_bias: dict[str, float] = Field(default_factory=dict, description="Systematic sector-level signal bias mapping")
    market_cap_bias: MarketCapBias = Field(description="Market capitalization quantile spread and bias")
    generated_at: str = Field(description="ISO-8601 UTC compilation timestamp")
    message: str = Field(description="Diagnostic and status message")


# Ergonomic alias
SignalQualityReport = SignalQualityReportResponse


# ═══════════════════════════════════════════════════════════════════════════════
# Point-in-Time (PIT) Certification & Audit Models
# ═══════════════════════════════════════════════════════════════════════════════

class PITCertificateParams(BaseModel):
    """Query parameters for requesting a Point-in-Time Data Certificate."""
    model_config = ConfigDict(extra="ignore")

    dataset_version: Optional[str] = Field(default="2.1.0", description="Dataset or processing pipeline version to certify")
    universe: Optional[str] = Field(default="all", description="Target stock universe to audit ('all', 'sp500', or comma-separated tickers)")
    start_date: Optional[str] = Field(default=None, description="Audit start date in YYYY-MM-DD format")
    end_date: Optional[str] = Field(default=None, description="Audit end date in YYYY-MM-DD format")


class PITTestResult(BaseModel):
    """Single point-in-time audit test evaluation result."""
    model_config = ConfigDict(extra="ignore")

    violations: int = Field(description="Count of detected look-ahead or temporal rule violations")
    total_checked: int = Field(description="Total records evaluated during audit")
    status: str = Field(description="Test execution status ('pass', 'conditional_pass', 'fail')")


class PITDuplicateTestResult(BaseModel):
    """Duplicate event test evaluation result."""
    model_config = ConfigDict(extra="ignore")

    duplicate_rate_pct: float = Field(description="Percentage of duplicate records detected")
    total_checked: int = Field(description="Total records evaluated")
    status: str = Field(description="Test execution status ('pass', 'fail')")


class PITBackfillTestResult(BaseModel):
    """Backfill consistency test evaluation result."""
    model_config = ConfigDict(extra="ignore")

    backfill_count: int = Field(description="Count of records backfilled into historical stream")
    policy: str = Field(description="Platform data backfill policy applied")
    status: str = Field(description="Test execution status ('pass', 'conditional_pass', 'fail')")


class PITCertificateTests(BaseModel):
    """Battery of 8 automated Point-in-Time and Look-Ahead Bias audit tests."""
    model_config = ConfigDict(extra="ignore")

    signal_availability_ordering: PITTestResult = Field(description="Triple timestamp ordering verification")
    ticker_rename: PITTestResult = Field(description="Ticker rename look-ahead prevention audit")
    delisted_security: PITTestResult = Field(description="Delisted security survivorship bias check")
    corporate_action: PITTestResult = Field(description="Corporate actions effective date alignment")
    duplicate_event: PITDuplicateTestResult = Field(description="Duplicate event deduplication rate")
    out_of_order_event: PITTestResult = Field(description="Clock skew / out-of-order event check")
    timestamp_precision: PITTestResult = Field(description="Timezone and timestamp precision verification")
    backfill_consistency: PITBackfillTestResult = Field(description="Backfill latency and SLA consistency check")


class PITCertificatePolicies(BaseModel):
    """Institutional governance, correction, and timestamp policies."""
    model_config = ConfigDict(extra="ignore")

    timestamp_policy: str = Field(description="Ingestion timestamp specification standard")
    correction_policy: str = Field(description="Historical data correction policy")
    backfill_policy: str = Field(description="Allowed backfill window and auditing requirements")
    universe_policy: str = Field(description="Survivorship bias mitigation policy")


class PITCertificateResponse(BaseModel):
    """Formal Cryptographically Signed Point-in-Time Audit Certificate."""
    model_config = ConfigDict(extra="ignore")

    certificate_id: str = Field(description="Unique deterministic certificate identification code")
    dataset_version: str = Field(description="Certified dataset or processing pipeline version")
    universe: str = Field(description="Evaluated asset universe filter")
    audit_start_date: str = Field(description="Historical audit start date (YYYY-MM-DD)")
    audit_end_date: str = Field(description="Historical audit end date (YYYY-MM-DD)")
    issued_at: str = Field(description="Certificate generation timestamp (ISO-8601 UTC)")
    overall_result: str = Field(description="Overall certification outcome ('pass', 'conditional_pass', 'fail')")
    tests: PITCertificateTests = Field(description="Comprehensive audit test results breakdown")
    policies: PITCertificatePolicies = Field(description="Platform governance and audit policies certified")
    signature: str = Field(description="Cryptographic SHA-256 digital signature over certificate payload")
    status: str = Field(description="Certificate active status ('active', 'revoked', 'expired')")
    archive_object_key: Optional[str] = Field(default=None, description="Archive object storage key for cryptographic audit proof")
    archive_timestamp: Optional[str] = Field(default=None, description="Timestamp when cryptographic proof was archived (ISO-8601 UTC)")



# Ergonomic alias
PITCertificate = PITCertificateResponse


class ProviderHealthItem(BaseModel):
    """Institutional data provider operational health and telemetry metrics."""
    model_config = ConfigDict(extra="ignore")

    provider: str = Field(description="Upstream data provider identifier ('sec_edgar', 'finnhub', 'polygon')")
    status: str = Field(description="Operational status ('healthy', 'degraded', 'outage')")
    requests_total: int = Field(description="Total requests processed within the monitoring window")
    requests_success: int = Field(description="Total successful requests processed within the monitoring window")
    success_rate_pct: float = Field(description="Calculated availability success rate percentage (0.0 to 100.0)")
    avg_latency_ms: float = Field(description="Mean request latency in milliseconds")
    p95_latency_ms: float = Field(description="95th percentile request latency in milliseconds")
    error_count_last_hour: int = Field(description="Number of request failures or errors observed in the past hour")
    last_success_timestamp: Optional[str] = Field(default=None, description="ISO-8601 timestamp of last successful upstream synchronization")
    last_error_message: Optional[str] = Field(default=None, description="Detailed message of the most recent upstream error or failure")
    quality_score: Optional[float] = Field(default=1.0, description="Data source quality score (0.0 to 1.0)")
    quarantine_count: Optional[int] = Field(default=0, description="Number of quarantined records for this provider")



class ProviderHealthResponse(BaseModel):
    """Aggregated upstream data provider operational health response."""
    model_config = ConfigDict(extra="ignore")

    providers: List[ProviderHealthItem] = Field(default_factory=list, description="List of provider health telemetry items")
    generated_at: str = Field(description="ISO-8601 UTC timestamp of health report generation")


# Ergonomic alias
ProviderHealth = ProviderHealthResponse


class PerClassMetrics(BaseModel):
    """Evaluation metrics for an individual sentiment class (positive, negative, neutral)."""
    model_config = ConfigDict(extra="ignore")

    precision: float = Field(description="Precision score: TP / (TP + FP)")
    recall: float = Field(description="Recall / Sensitivity score: TP / (TP + FN)")
    f1_score: float = Field(description="Harmonic mean of precision and recall")
    support: int = Field(description="Total ground-truth sample count for this class")


class ClassConfusion(BaseModel):
    """Prediction distribution for a specific true ground-truth class."""
    model_config = ConfigDict(extra="ignore")

    predicted_positive: int = Field(default=0, description="Count predicted as positive")
    predicted_negative: int = Field(default=0, description="Count predicted as negative")
    predicted_neutral: int = Field(default=0, description="Count predicted as neutral")


class ConfusionMatrix(BaseModel):
    """Full 3x3 Confusion Matrix across sentiment classes."""
    model_config = ConfigDict(extra="ignore")

    true_positive: ClassConfusion = Field(description="Predictions for ground-truth positive samples")
    true_negative: ClassConfusion = Field(description="Predictions for ground-truth negative samples")
    true_neutral: ClassConfusion = Field(description="Predictions for ground-truth neutral samples")


class ClassificationMetrics(BaseModel):
    """Aggregated multi-class classification metrics."""
    model_config = ConfigDict(extra="ignore")

    accuracy: float = Field(description="Overall multi-class classification accuracy (0.0 to 1.0)")
    macro_precision: float = Field(description="Unweighted macro-averaged precision across all 3 classes")
    macro_recall: float = Field(description="Unweighted macro-averaged recall across all 3 classes")
    macro_f1: float = Field(description="Unweighted macro-averaged F1 score across all 3 classes")
    positive: PerClassMetrics = Field(description="Per-class performance breakdown for positive sentiment")
    negative: PerClassMetrics = Field(description="Per-class performance breakdown for negative sentiment")
    neutral: PerClassMetrics = Field(description="Per-class performance breakdown for neutral sentiment")


class CalibrationPoint(BaseModel):
    """Decile bin for probability calibration analysis."""
    model_config = ConfigDict(extra="ignore")

    bin_index: int = Field(description="Decile bin index (0 to 9)")
    confidence_min: float = Field(description="Lower confidence bound of the bin [inclusive]")
    confidence_max: float = Field(description="Upper confidence bound of the bin [exclusive, or 1.0 inclusive]")
    predicted_confidence_mean: float = Field(description="Arithmetic mean of predicted confidence within the bin")
    accuracy: float = Field(description="Empirical classification accuracy within the bin")
    sample_count: int = Field(description="Total samples evaluated within this confidence decile")


class ModelValidationResponse(BaseModel):
    """Institutional Model Validation & Calibration Report response payload."""
    model_config = ConfigDict(extra="ignore")

    model_id: str = Field(description="Canonical model identifier")
    model_version: str = Field(description="Current production model artifact release version")
    dataset_version: str = Field(description="Version tag of the ground-truth benchmark evaluation dataset")
    dataset_size: int = Field(description="Total number of labeled financial headlines evaluated")
    evaluated_at: str = Field(description="ISO-8601 UTC timestamp when model validation was executed")
    metrics: ClassificationMetrics = Field(description="Comprehensive classification performance metrics")
    confusion_matrix: ConfusionMatrix = Field(description="3x3 multi-class confusion matrix breakdown")
    calibration_curve: List[CalibrationPoint] = Field(default_factory=list, description="Decile probability calibration curve points")
    brier_score: float = Field(description="Multi-class Brier score evaluating probabilistic prediction accuracy")
    expected_calibration_error: float = Field(description="Expected Calibration Error (ECE)")
    notes: List[str] = Field(default_factory=list, description="Contextual operational notes and validation conclusions")


# Ergonomic alias
ModelValidation = ModelValidationResponse















































