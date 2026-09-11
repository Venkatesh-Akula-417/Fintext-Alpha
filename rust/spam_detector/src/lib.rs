//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust Social Spam & Bot Network Detector
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! sub-second sliding-window spam and market manipulation detection for
//! mid-frequency social media sentiment streams (Reddit, StockTwits, Twitter/X).
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use pyo3::prelude::*;
use regex::Regex;
use std::collections::VecDeque;
use std::sync::Mutex;

static PUMP_REGEXES: Lazy<Vec<Regex>> = Lazy::new(|| {
    let patterns = vec![
        r"(?i)\bto the moon\b",
        r"(?i)\b100x\b",
        r"(?i)\b10x\b",
        r"(?i)\b1000x\b",
        r"(?i)\bguaranteed\b",
        r"(?i)\binsider\b",
        r"(?i)\bdon't miss\b",
        r"(?i)\bdont miss\b",
        r"(?i)\bpump\b",
        r"(?i)\bdump\b",
        r"(?i)\brocket\b",
        r"(?i)\byolo\b",
        r"(?i)\bshort squeeze\b",
        r"(?i)\bdiamond hands\b",
        r"(?i)\bmoon\b",
        r"(?i)\bbuy now\b",
        r"(?i)\bmassive gains\b",
        r"(?i)\bpumping\b",
        r"(?i)\bnext gamestop\b",
    ];

    patterns
        .into_iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
});

const PUMP_EMOJIS: &[&str] = &["🚀", "💎", "🦍", "🌕", "💸", "🔥"];

fn normalize_title(title: &str) -> String {
    let lowered = title.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn count_promotional_cues(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }

    let mut matches = 0;
    for r in PUMP_REGEXES.iter() {
        matches += r.find_iter(text).count();
    }

    for emoji in PUMP_EMOJIS.iter() {
        matches += text.matches(emoji).count();
    }

    matches
}

#[pyclass]
pub struct RustSpamDetector {
    window_seconds: f64,
    duplicate_threshold: usize,
    burst_threshold: usize,
    title_window: Mutex<VecDeque<(f64, String)>>,
    burst_window: Mutex<VecDeque<(f64, String)>>,
}

#[pymethods]
impl RustSpamDetector {
    #[new]
    #[pyo3(signature = (window_seconds=60.0, duplicate_threshold=3, burst_threshold=5))]
    pub fn new(
        window_seconds: Option<f64>,
        duplicate_threshold: Option<usize>,
        burst_threshold: Option<usize>,
    ) -> Self {
        RustSpamDetector {
            window_seconds: window_seconds.unwrap_or(60.0),
            duplicate_threshold: duplicate_threshold.unwrap_or(3),
            burst_threshold: burst_threshold.unwrap_or(5),
            title_window: Mutex::new(VecDeque::new()),
            burst_window: Mutex::new(VecDeque::new()),
        }
    }

    /// Evaluate whether an incoming post represents spam, duplicate, or coordinated burst.
    pub fn check_spam(&self, title: &str, source: &str, timestamp: Option<f64>) -> (bool, String) {
        let now = timestamp.unwrap_or(0.0);
        let mut reasons: Vec<&'static str> = Vec::new();

        let norm_title = normalize_title(title);
        let burst_key = source.trim().to_lowercase();

        let cutoff = now - self.window_seconds;

        // Acquire lock and prune sliding windows
        if let (Ok(mut t_win), Ok(mut b_win)) = (self.title_window.lock(), self.burst_window.lock())
        {
            while let Some(front) = t_win.front() {
                if front.0 < cutoff {
                    t_win.pop_front();
                } else {
                    break;
                }
            }

            while let Some(front) = b_win.front() {
                if front.0 < cutoff {
                    b_win.pop_front();
                } else {
                    break;
                }
            }

            if !norm_title.is_empty() {
                t_win.push_back((now, norm_title.clone()));
            }

            if !burst_key.is_empty() {
                b_win.push_back((now, burst_key.clone()));
            }

            // 1. Duplicate Content Detection (> threshold in window)
            if !norm_title.is_empty() {
                let dup_count = t_win.iter().filter(|(_, t)| t == &norm_title).count();
                if dup_count > self.duplicate_threshold {
                    reasons.push("duplicate_content");
                }
            }

            // 2. Burst Posting Detection (> threshold in window)
            if !burst_key.is_empty() {
                let burst_count = b_win.iter().filter(|(_, k)| k == &burst_key).count();
                if burst_count > self.burst_threshold {
                    reasons.push("burst_posting");
                }
            }
        }

        // 3. Promotional Pump Language (>= 3 phrases/emojis)
        let promo_count = count_promotional_cues(title);
        if promo_count >= 3 {
            reasons.push("promotional_language");
        }

        if !reasons.is_empty() {
            (true, reasons.join(","))
        } else {
            (false, String::new())
        }
    }

    /// Reset internal sliding window state.
    pub fn reset(&self) {
        if let Ok(mut t_win) = self.title_window.lock() {
            t_win.clear();
        }
        if let Ok(mut b_win) = self.burst_window.lock() {
            b_win.clear();
        }
    }
}

/// PyO3 Module Definition
#[pymodule]
fn fintext_spam_detector(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustSpamDetector>()?;
    Ok(())
}
