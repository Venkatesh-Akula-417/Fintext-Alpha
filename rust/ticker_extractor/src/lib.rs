//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust Ticker Extractor & Entity Disambiguator
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! sub-second ticker symbol identification from cashtags, exchange prefixes,
//! standalone symbols, and corporate lexicons with context-aware disambiguation.
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use pyo3::prelude::*;
use regex::Regex;
use std::collections::BTreeMap;

static CASHTAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$([A-Za-z0-9.]{1,6})\b").unwrap());

static EXCHANGE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:NASDAQ|NYSE|AMEX|NSE|BSE|TADAWUL|ticker):([A-Za-z0-9.]{1,6})\b").unwrap()
});

struct CompanyMapping {
    name: &'static str,
    ticker: &'static str,
    regex: Regex,
    is_ambiguous: bool,
}

static COMPANY_LEXICON: Lazy<Vec<CompanyMapping>> = Lazy::new(|| {
    let raw_defs: Vec<(&'static str, &'static str, bool)> = vec![
        ("berkshire hathaway", "BRK.B", false),
        ("bank of america", "BAC", false),
        ("goldman sachs", "GS", false),
        ("morgan stanley", "MS", false),
        ("johnson & johnson", "JNJ", false),
        ("reliance industries", "RELIANCE", false),
        ("tata consultancy", "TCS", false),
        ("coca cola", "KO", false),
        ("coca-cola", "KO", false),
        ("jp morgan", "JPM", false),
        ("jpmorgan", "JPM", false),
        ("salesforce", "CRM", false),
        ("microsoft", "MSFT", false),
        ("alphabet", "GOOGL", false),
        ("broadcom", "AVGO", false),
        ("qualcomm", "QCOM", false),
        ("citigroup", "C", false),
        ("wells fargo", "WFC", false),
        ("mastercard", "MA", false),
        ("walmart", "WMT", false),
        ("chevron", "CVX", false),
        ("palantir", "PLTR", false),
        ("infosys", "INFY", false),
        ("netflix", "NFLX", false),
        ("oracle", "ORCL", false),
        ("pepsico", "PEP", false),
        ("nvidia", "NVDA", false),
        ("google", "GOOGL", false),
        ("pfizer", "PFE", false),
        ("amazon", "AMZN", true),
        ("apple", "AAPL", true),
        ("tesla", "TSLA", true),
        ("adobe", "ADBE", false),
        ("intel", "INTC", false),
        ("cisco", "CSCO", false),
        ("merck", "MRK", false),
        ("exxon", "XOM", false),
        ("pepsi", "PEP", false),
        ("meta", "META", true),
        ("visa", "V", true),
        ("amd", "AMD", false),
        ("ibm", "IBM", false),
        ("tcs", "TCS", false),
    ];

    raw_defs
        .into_iter()
        .map(|(name, ticker, is_ambiguous)| {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(name));
            CompanyMapping {
                name,
                ticker,
                regex: Regex::new(&pattern).unwrap(),
                is_ambiguous,
            }
        })
        .collect()
});

static AMAZON_DISAMBIGUATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\bamazon\s+(river|rainforest|basin|jungle|forest|tribe)\b").unwrap()
});

static TESLA_DISAMBIGUATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:\btesla\s+(the\s+inventor|nikola)\b|nikola\s+tesla)").unwrap()
});

static APPLE_DISAMBIGUATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:\b(?:red|green|ripe|fresh|delicious|sliced|peeled)\s+apple\b|\bapple\s+(?:pie|tree|sauce|cider|orchard|juice)\b|\b(?:ate|eat|eating|eats)\s+an?\s+apple\b)").unwrap()
});
fn is_valid_ticker_symbol(symbol: &str) -> bool {
    let sym = symbol.to_uppercase();
    if sym.is_empty() || sym.len() > 6 {
        return false;
    }
    if !sym.chars().any(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    if sym.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        return false;
    }
    true
}

fn extract_tickers_with_scores(text: &str) -> BTreeMap<String, f64> {
    let mut tickers: BTreeMap<String, f64> = BTreeMap::new();
    if text.trim().is_empty() {
        return tickers;
    }

    // 1. Cashtags ($AAPL) -> Confidence 1.0
    for cap in CASHTAG_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let sym = m.as_str().to_uppercase();
            if is_valid_ticker_symbol(&sym) {
                tickers.insert(sym, 1.0);
            }
        }
    }

    // 2. Exchange Prefixes (NASDAQ:AAPL) -> Confidence 1.0
    for cap in EXCHANGE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let sym = m.as_str().to_uppercase();
            if is_valid_ticker_symbol(&sym) {
                tickers.insert(sym, 1.0);
            }
        }
    }

    // 3. Company Name Lexicon with Disambiguation -> Confidence 0.9
    for comp in COMPANY_LEXICON.iter() {
        if tickers.contains_key(comp.ticker) {
            continue;
        }

        if comp.regex.is_match(text) {
            let mut rejected = false;
            if comp.is_ambiguous {
                if comp.name == "amazon" && AMAZON_DISAMBIGUATION.is_match(text) {
                    rejected = true;
                } else if comp.name == "tesla" && TESLA_DISAMBIGUATION.is_match(text) {
                    rejected = true;
                } else if comp.name == "apple" && APPLE_DISAMBIGUATION.is_match(text) {
                    rejected = true;
                }
            }

            if !rejected {
                tickers.entry(comp.ticker.to_string()).or_insert(0.9);
            }
        }
    }

    tickers
}

pub fn extract_tickers(text: &str) -> Vec<String> {
    let scored = extract_tickers_with_scores(text);
    scored.into_keys().collect()
}

pub fn extract_tickers_with_confidence(text: &str) -> Vec<(String, f64)> {
    let scored = extract_tickers_with_scores(text);
    scored.into_iter().collect()
}

#[pyclass]
pub struct RustTickerExtractor;

#[pymethods]
impl RustTickerExtractor {
    #[new]
    pub fn new() -> Self {
        RustTickerExtractor
    }

    /// Extract sorted, deduplicated stock ticker symbols from text.
    pub fn extract_tickers(&self, text: &str) -> Vec<String> {
        let scored = extract_tickers_with_scores(text);
        scored.into_keys().collect()
    }

    /// Extract stock ticker symbols with confidence score (1.0 = explicit cashtag/exchange, 0.9 = NER lexicon).
    pub fn extract_tickers_with_confidence(&self, text: &str) -> Vec<(String, f64)> {
        let scored = extract_tickers_with_scores(text);
        scored.into_iter().collect()
    }
}

/// PyO3 Module Definition
#[pymodule]
fn fintext_ticker_extractor(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustTickerExtractor>()?;
    Ok(())
}
