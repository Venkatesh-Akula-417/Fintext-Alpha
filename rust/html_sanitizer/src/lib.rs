//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust HTML Sanitizer & Text Extractor
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! High-throughput HTML stripping, boilerplate reduction, and URL extraction
//! engine for financial RSS, news, and regulatory filing feeds.
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use pyo3::prelude::*;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;

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

static LINK_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("a[href]").unwrap_or_else(|_| Selector::parse("a").unwrap()));

static WHITESPACE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

pub fn clean_html(html: &str) -> String {
    clean_html_internal(html)
}

pub fn extract_links(html: &str) -> Vec<String> {
    extract_links_internal(html)
}

fn clean_html_internal(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    let fragment = Html::parse_fragment(html);
    let mut text_chunks = Vec::new();

    for node in fragment.tree.nodes() {
        if let Some(elem) = node.value().as_element() {
            let tag_name = elem.name().to_lowercase();
            if IGNORED_TAGS.contains(tag_name.as_str()) {
                continue;
            }
        }
        if let Some(text_node) = node.value().as_text() {
            // Check if any ancestor is in IGNORED_TAGS
            let mut is_ignored = false;
            let mut current_parent = node.parent();
            while let Some(parent_ref) = current_parent {
                if let Some(p_elem) = parent_ref.value().as_element() {
                    if IGNORED_TAGS.contains(p_elem.name().to_lowercase().as_str()) {
                        is_ignored = true;
                        break;
                    }
                }
                current_parent = parent_ref.parent();
            }

            if !is_ignored {
                let chunk = text_node.trim();
                if !chunk.is_empty() {
                    text_chunks.push(chunk);
                }
            }
        }
    }

    let combined = text_chunks.join(" ");
    WHITESPACE_REGEX
        .replace_all(&combined, " ")
        .trim()
        .to_string()
}

fn extract_links_internal(html: &str) -> Vec<String> {
    if html.trim().is_empty() {
        return Vec::new();
    }

    let document = Html::parse_fragment(html);
    let mut links = Vec::new();
    let mut seen = HashSet::new();

    for element in document.select(&LINK_SELECTOR) {
        if let Some(href) = element.value().attr("href") {
            let clean_href = href.trim();
            if !clean_href.is_empty()
                && !clean_href.starts_with('#')
                && !clean_href.starts_with("javascript:")
                && seen.insert(clean_href.to_string())
            {
                links.push(clean_href.to_string());
            }
        }
    }

    links
}

#[derive(Default)]
#[pyclass]
pub struct RustHtmlSanitizer;

#[pymethods]
impl RustHtmlSanitizer {
    #[new]
    pub fn new() -> Self {
        RustHtmlSanitizer
    }

    /// Extract clean, noise-free text from raw HTML markup.
    pub fn clean_text(&self, html: &str) -> String {
        clean_html_internal(html)
    }

    /// Extract deduplicated hyperlink targets from raw HTML markup.
    pub fn extract_links(&self, html: &str) -> Vec<String> {
        extract_links_internal(html)
    }

    /// Extract both clean plaintext and hyperlink targets in a single pass.
    pub fn extract_text_and_links(&self, html: &str) -> (String, Vec<String>) {
        (clean_html_internal(html), extract_links_internal(html))
    }
}

/// PyO3 Module Definition
#[pymodule]
fn fintext_html_sanitizer(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustHtmlSanitizer>()?;
    Ok(())
}
