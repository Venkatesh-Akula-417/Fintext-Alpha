//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Standalone Rust Preprocessing Sidecar
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides an ultra-high-throughput, zero-GIL REST/IPC microservice for
//! parallel HTML sanitization, stock ticker extraction, and spam detection.
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use rayon::prelude::*;
use regex::Regex;
use scraper::Html;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use warp::Filter;

// ─────────────────────────────────────────────────────────────────────────────
// HTML Sanitization Engine
// ─────────────────────────────────────────────────────────────────────────────
static IGNORED_TAGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut s = HashSet::new();
    s.insert("script");
    s.insert("style");
    s.insert("noscript");
    s.insert("iframe");
    s.insert("svg");
    s.insert("nav");
    s.insert("footer");
    s.insert("header");
    s.insert("aside");
    s.insert("form");
    s.insert("button");
    s.insert("input");
    s.insert("select");
    s.insert("textarea");
    s.insert("img");
    s
});

static WHITESPACE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

fn clean_html_internal(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }
    let fragment = Html::parse_fragment(html);
    let mut text_chunks = Vec::new();

    for node in fragment.tree.nodes() {
        if let Some(elem) = node.value().as_element() {
            let tag = elem.name().to_lowercase();
            if IGNORED_TAGS.contains(tag.as_str()) {
                continue;
            }
        }
        if let Some(text_node) = node.value().as_text() {
            let chunk = text_node.text.trim();
            if !chunk.is_empty() {
                text_chunks.push(chunk);
            }
        }
    }
    let joined = text_chunks.join(" ");
    WHITESPACE_REGEX
        .replace_all(&joined, " ")
        .trim()
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Ticker Extraction Engine
// ─────────────────────────────────────────────────────────────────────────────
static CASHTAG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$([A-Za-z0-9.]{1,6})\b").unwrap());
static EXCHANGE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:NASDAQ|NYSE|AMEX|NSE|BSE|TADAWUL|ticker):([A-Za-z0-9.]{1,6})\b").unwrap()
});

fn extract_tickers_internal(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();

    for cap in CASHTAG_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let sym = m.as_str().to_uppercase();
            if seen.insert(sym.clone()) {
                found.push(sym);
            }
        }
    }
    for cap in EXCHANGE_REGEX.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            let sym = m.as_str().to_uppercase();
            if seen.insert(sym.clone()) {
                found.push(sym);
            }
        }
    }
    found
}

// ─────────────────────────────────────────────────────────────────────────────
// Spam Detection Engine
// ─────────────────────────────────────────────────────────────────────────────
static SPAM_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        Regex::new(r"(?i)\b(?:buy now|click here|guaranteed returns|1000% profit|join telegram)\b")
            .unwrap(),
        Regex::new(r"(?i)\b(?:pump and dump|moon shot|whatsapp group|free crypto)\b").unwrap(),
    ]
});

fn is_spam_internal(text: &str) -> (bool, Option<String>) {
    for pat in SPAM_PATTERNS.iter() {
        if pat.is_match(text) {
            return (true, Some(pat.as_str().to_string()));
        }
    }
    (false, None)
}

// ─────────────────────────────────────────────────────────────────────────────
// DTOs
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
struct BatchTextRequest {
    texts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SanitizeResult {
    clean_text: String,
    original_len: usize,
    clean_len: usize,
}

#[derive(Debug, Serialize)]
struct ProcessResult {
    clean_text: String,
    tickers: Vec<String>,
    is_spam: bool,
    spam_reason: Option<String>,
    event_type: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Application Entrypoint
// ─────────────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    let port = std::env::var("SIDECAR_PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse::<u16>()
        .unwrap_or(8081);

    println!("Starting FinText Rust Sidecar on port {}...", port);

    // GET /health
    let health_route = warp::path("health").and(warp::get()).map(|| {
        warp::reply::json(&HealthResponse {
            status: "healthy".to_string(),
            service: "fintext-rust-sidecar".to_string(),
            version: "0.1.0".to_string(),
        })
    });

    // POST /sanitize_batch
    let sanitize_route = warp::path("sanitize_batch")
        .and(warp::post())
        .and(warp::body::json())
        .map(|req: BatchTextRequest| {
            let results: Vec<SanitizeResult> = req
                .texts
                .par_iter()
                .map(|t| {
                    let cleaned = clean_html_internal(t);
                    SanitizeResult {
                        original_len: t.len(),
                        clean_len: cleaned.len(),
                        clean_text: cleaned,
                    }
                })
                .collect();
            warp::reply::json(&results)
        });

    // POST /extract_tickers_batch
    let tickers_route = warp::path("extract_tickers_batch")
        .and(warp::post())
        .and(warp::body::json())
        .map(|req: BatchTextRequest| {
            let results: Vec<Vec<String>> = req
                .texts
                .par_iter()
                .map(|t| {
                    let cleaned = clean_html_internal(t);
                    extract_tickers_internal(&cleaned)
                })
                .collect();
            warp::reply::json(&results)
        });

    // POST /process_batch
    let process_route = warp::path("process_batch")
        .and(warp::post())
        .and(warp::body::json())
        .map(|req: BatchTextRequest| {
            let results: Vec<ProcessResult> = req
                .texts
                .par_iter()
                .map(|t| {
                    let clean_text = clean_html_internal(t);
                    let tickers = extract_tickers_internal(&clean_text);
                    let (is_spam, spam_reason) = is_spam_internal(&clean_text);
                    ProcessResult {
                        clean_text,
                        tickers,
                        is_spam,
                        spam_reason,
                        event_type: "Other".to_string(),
                    }
                })
                .collect();
            warp::reply::json(&results)
        });

    let routes = health_route
        .or(sanitize_route)
        .or(tickers_route)
        .or(process_route);

    warp::serve(routes).run(([127, 0, 0, 1], port)).await;
}
