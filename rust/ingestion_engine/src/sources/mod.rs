//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Active Ingestion Source Modules
//! ═══════════════════════════════════════════════════════════════════════════════

pub mod corporate_actions_updater;
pub mod finnhub;
pub mod finnhub_ws;
pub mod fomc;
pub mod polygon;
pub mod polygon_ws;
pub mod sec_edgar;

pub use corporate_actions_updater::*;
pub use finnhub::{parse_finnhub_news_json, FinnhubArticle, FinnhubClient};
pub use finnhub_ws::{FinnhubWsClient, FinnhubWsConfig, FinnhubWsMessage, FinnhubWsNewsItem};
pub use fomc::FomcFetcher;
pub use polygon::{
    parse_options_ticker, DailyBar, OptionTrade, ParsedOptionsContract, PolygonAggsResponse,
    PolygonClient,
};
pub use polygon_ws::{PolygonWsClient, PolygonWsConfig, PolygonWsMessage, PolygonWsRawTrade};
pub use sec_edgar::SecEdgarFetcher;
