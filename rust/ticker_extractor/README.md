# FinText Ticker Extractor (`fintext_ticker_extractor`)

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../../docs/DEPRECATED.md)

## 1. Module Name & Purpose
`fintext_ticker_extractor` is a high-speed stock ticker extraction and named entity disambiguation engine. It identifies stock symbols from cashtags (`$AAPL`), exchange-prefixed identifiers (`NASDAQ:NVDA`), and corporate lexicons while eliminating common English false positives through negative disambiguation rules.

## 2. Architecture
- **Regex Extraction**:
  - `CASHTAG_REGEX`: Identifies 1-6 character uppercase alphanumeric cashtags.
  - `EXCHANGE_REGEX`: Captures symbols with exchange qualifiers (e.g. `NASDAQ:`, `NYSE:`, `AMEX:`, `NSE:`, `BSE:`).
- **Lexicon Lookup**: Matches company names (e.g., Berkshire Hathaway -> `BRK.B`, Alphabet -> `GOOGL`, Microsoft -> `MSFT`) using `COMPANY_LEXICON`.
- **Negative Disambiguation**: Applies context filters to prune common polysemous words:
  - *Amazon*: Excludes geographic references (river, rainforest, jungle).
  - *Tesla*: Excludes biographical references (the inventor, Nikola).
  - *Apple*: Excludes botanical and food references (pie, orchard, cider, ate an apple).
- **Confidence Scoring**: Assigns calibrated confidence scores (e.g. `1.0` for exchange prefixes, `0.95` for cashtags, `0.85` for unambiguous company names).

## 3. Dependencies
| Dependency | Version | Purpose |
| :--- | :--- | :--- |
| `regex` | `1.10` | Compiled regular expression matching |
| `once_cell` | `1.19` | Lazy initialization of compiled regexes and lexicon tables |
| `pyo3` | `0.29` | Optional Python C-extension bindings (`cdylib`) |

## 4. Public API
- `pub fn extract_tickers(text: &str) -> Vec<String>`: Returns deduplicated ticker symbols.
- `pub fn extract_tickers_with_confidence(text: &str) -> Vec<(String, f64)>`: Returns symbols paired with confidence scores.
- `pub struct RustTickerExtractor`: PyO3 class wrapper for Python callers.

## 5. Testing
Run crate unit tests:
```bash
cargo test -p fintext_ticker_extractor
```

## 6. Usage Example
```rust
use fintext_ticker_extractor::extract_tickers_with_confidence;

let text = "Apple and Microsoft announce major partnership; $TSLA rises on NASDAQ:NVDA surge";
let results = extract_tickers_with_confidence(text);

for (ticker, conf) in results {
    println!("Found {ticker} (confidence: {conf:.2})");
}
```

## 7. Related Modules
- [`fintext_ingestion_engine`](../ingestion_engine): Extracts symbols to route documents into microstructure and GNN pipelines.
- [`fintext_api_server`](../api_server): Provides ticker resolution and mapping.
