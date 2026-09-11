//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Entity Sentiment Breakdown Handler
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Provides aggregated sentiment metrics across extracted financial named entities
//! (Companies, Executives/Persons, Products, Organizations, Locations) mentioned in
//! news articles and SEC filings (`GET /sentiment/entities`).
//! ═══════════════════════════════════════════════════════════════════════════════

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde_json::json;
use std::collections::HashMap;

use crate::models::{EntitySentimentItem, EntitySentimentResponse, SentimentEntitiesParams};
use crate::state::AppState;

pub const ALLOWED_ENTITY_TYPES: &[&str] = &[
    "company",
    "person",
    "product",
    "location",
    "organization",
    "all",
];

pub const ALLOWED_SORT_FIELDS: &[&str] = &[
    "avg_sentiment",
    "mentions",
    "positive_ratio",
    "negative_ratio",
];

/// Helper to parse a date string supporting `YYYY-MM-DD` or RFC3339 formats.
fn parse_date(date_str: &str) -> Result<NaiveDate, String> {
    let trimmed = date_str.trim();
    if trimmed.is_empty() {
        return Err("Date string cannot be empty".to_string());
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.naive_utc().date());
    }

    if let Ok(nd) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return Ok(nd);
    }

    Err(format!(
        "Invalid date format '{}'. Expected YYYY-MM-DD (e.g. '2026-08-30') or RFC3339 timestamp",
        trimmed
    ))
}

/// Representation of an entity mention in a specific article.
#[derive(Debug, Clone)]
struct RawEntityMention {
    entity_text: &'static str,
    entity_type: &'static str,
    sentiment_score: f64,
    date: &'static str, // YYYY-MM-DD
}

/// Generates mock news article entity mentions for development and tests.
fn get_mock_entity_mentions() -> Vec<RawEntityMention> {
    vec![
        // ─── Apple (Company) ───
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.85,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.72,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.40,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.65,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.15,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: -0.20,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.50,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.35,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.60,
            date: "2026-08-21",
        },
        RawEntityMention {
            entity_text: "Apple",
            entity_type: "company",
            sentiment_score: 0.45,
            date: "2026-08-20",
        },
        // ─── Nvidia (Company) ───
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.92,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.88,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.75,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.80,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.68,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.85,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.70,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.90,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.78,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.82,
            date: "2026-08-20",
        },
        RawEntityMention {
            entity_text: "Nvidia",
            entity_type: "company",
            sentiment_score: 0.64,
            date: "2026-08-18",
        },
        // ─── Microsoft (Company) ───
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.62,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.58,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.45,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.70,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.50,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.40,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.55,
            date: "2026-08-21",
        },
        RawEntityMention {
            entity_text: "Microsoft",
            entity_type: "company",
            sentiment_score: 0.65,
            date: "2026-08-19",
        },
        // ─── Tesla (Company) ───
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.45,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.30,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: 0.20,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.50,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.15,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: 0.10,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.40,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Tesla",
            entity_type: "company",
            sentiment_score: -0.25,
            date: "2026-08-20",
        },
        // ─── Amazon (Company) ───
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.38,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.42,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.30,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.50,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.25,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Amazon",
            entity_type: "company",
            sentiment_score: 0.35,
            date: "2026-08-21",
        },
        // ─── Alphabet (Company) ───
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.52,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.48,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.35,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.60,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.40,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Alphabet",
            entity_type: "company",
            sentiment_score: 0.30,
            date: "2026-08-20",
        },
        // ─── Tim Cook (Person) ───
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.65,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.50,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.40,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.55,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.30,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.45,
            date: "2026-08-21",
        },
        RawEntityMention {
            entity_text: "Tim Cook",
            entity_type: "person",
            sentiment_score: 0.60,
            date: "2026-08-20",
        },
        // ─── Jensen Huang (Person) ───
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.95,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.85,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.80,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.90,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.75,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.88,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Jensen Huang",
            entity_type: "person",
            sentiment_score: 0.82,
            date: "2026-08-20",
        },
        // ─── Satya Nadella (Person) ───
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.70,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.65,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.55,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.60,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.45,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Satya Nadella",
            entity_type: "person",
            sentiment_score: 0.58,
            date: "2026-08-20",
        },
        // ─── Elon Musk (Person) ───
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.35,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.40,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: 0.10,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.45,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.20,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.10,
            date: "2026-08-21",
        },
        RawEntityMention {
            entity_text: "Elon Musk",
            entity_type: "person",
            sentiment_score: -0.30,
            date: "2026-08-19",
        },
        // ─── Blackwell GPU (Product) ───
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.94,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.88,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.82,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.90,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.76,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.85,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Blackwell GPU",
            entity_type: "product",
            sentiment_score: 0.79,
            date: "2026-08-20",
        },
        // ─── iPhone 17 (Product) ───
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.75,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.60,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.45,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.50,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.35,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "iPhone 17",
            entity_type: "product",
            sentiment_score: 0.40,
            date: "2026-08-20",
        },
        // ─── Azure AI (Product) ───
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.68,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.62,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.55,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.72,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.58,
            date: "2026-08-22",
        },
        RawEntityMention {
            entity_text: "Azure AI",
            entity_type: "product",
            sentiment_score: 0.64,
            date: "2026-08-20",
        },
        // ─── Full Self-Driving (Product) ───
        RawEntityMention {
            entity_text: "Full Self-Driving",
            entity_type: "product",
            sentiment_score: -0.40,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Full Self-Driving",
            entity_type: "product",
            sentiment_score: -0.35,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Full Self-Driving",
            entity_type: "product",
            sentiment_score: 0.15,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Full Self-Driving",
            entity_type: "product",
            sentiment_score: -0.50,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Full Self-Driving",
            entity_type: "product",
            sentiment_score: -0.20,
            date: "2026-08-21",
        },
        // ─── SEC (Organization) ───
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.55,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.40,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.30,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.60,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.25,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.45,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "SEC",
            entity_type: "organization",
            sentiment_score: -0.50,
            date: "2026-08-20",
        },
        // ─── Federal Reserve (Organization) ───
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: 0.10,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: -0.15,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: -0.20,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: 0.05,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: -0.10,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: -0.30,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Federal Reserve",
            entity_type: "organization",
            sentiment_score: -0.05,
            date: "2026-08-20",
        },
        // ─── OpenAI (Organization) ───
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.65,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.55,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.40,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.70,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.45,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "OpenAI",
            entity_type: "organization",
            sentiment_score: 0.50,
            date: "2026-08-23",
        },
        // ─── Cupertino (Location) ───
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.45,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.35,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.25,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.50,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.30,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Cupertino",
            entity_type: "location",
            sentiment_score: 0.40,
            date: "2026-08-21",
        },
        // ─── Santa Clara (Location) ───
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.85,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.75,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.80,
            date: "2026-08-28",
        },
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.70,
            date: "2026-08-26",
        },
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.65,
            date: "2026-08-24",
        },
        RawEntityMention {
            entity_text: "Santa Clara",
            entity_type: "location",
            sentiment_score: 0.78,
            date: "2026-08-23",
        },
        // ─── Austin (Location) ───
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: -0.10,
            date: "2026-08-30",
        },
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: -0.20,
            date: "2026-08-29",
        },
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: 0.15,
            date: "2026-08-27",
        },
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: -0.30,
            date: "2026-08-25",
        },
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: 0.05,
            date: "2026-08-23",
        },
        RawEntityMention {
            entity_text: "Austin",
            entity_type: "location",
            sentiment_score: -0.15,
            date: "2026-08-21",
        },
    ]
}

/// Handler for `GET /sentiment/entities`
#[utoipa::path(
    get,
    path = "/sentiment/entities",
    tag = "Sentiment & Analytics",
    params(
        SentimentEntitiesParams
    ),
    responses(
        (status = 200, description = "Aggregated entity sentiment metrics returned successfully", body = EntitySentimentResponse),
        (status = 400, description = "Invalid query parameters (dates, bounds, or sorting)"),
        (status = 401, description = "Unauthorized - Missing or invalid Bearer token")
    )
)]
pub async fn get_sentiment_entities_handler(
    State(state): State<AppState>,
    Query(params): Query<SentimentEntitiesParams>,
) -> Response {
    let model_version = Some(state.get_model_version());
    let pipeline_version = Some(state.get_pipeline_version());
    let data_provenance = Some(state.get_data_provenance());
    // 1. Validate & Parse Limit
    let limit = match params.limit {
        Some(l) => {
            if l == 0 || l > 100 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": "limit must be between 1 and 100"
                    })),
                )
                    .into_response();
            }
            l
        }
        None => 20,
    };

    // 2. Validate & Parse min_mentions
    let min_mentions = match params.min_mentions {
        Some(m) => {
            if m == 0 {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": "min_mentions must be at least 1"
                    })),
                )
                    .into_response();
            }
            m
        }
        None => 5,
    };

    // 3. Validate entity_type
    let raw_entity_type = params.entity_type.as_deref().unwrap_or("all");
    let entity_type = raw_entity_type.trim().to_lowercase();
    if !ALLOWED_ENTITY_TYPES.contains(&entity_type.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid entity_type '{}'. Allowed values: {:?}",
                    raw_entity_type, ALLOWED_ENTITY_TYPES
                )
            })),
        )
            .into_response();
    }

    // 4. Validate sort_by
    let raw_sort_by = params.sort_by.as_deref().unwrap_or("avg_sentiment");
    let sort_by = raw_sort_by.trim().to_lowercase();
    if !ALLOWED_SORT_FIELDS.contains(&sort_by.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "Invalid sort_by '{}'. Allowed values: {:?}",
                    raw_sort_by, ALLOWED_SORT_FIELDS
                )
            })),
        )
            .into_response();
    }

    // 5. Validate & Parse Date Range
    let today = Utc::now().naive_utc().date();
    let default_end = today;
    let default_start = default_end - Duration::days(30);

    let start_date = match params.start_date.as_deref() {
        Some(s) => match parse_date(s) {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": format!("Invalid start_date: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => default_start,
    };

    let end_date = match params.end_date.as_deref() {
        Some(s) => match parse_date(s) {
            Ok(d) => d,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "Bad Request",
                        "message": format!("Invalid end_date: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => default_end,
    };

    if start_date > end_date {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Bad Request",
                "message": format!(
                    "start_date ({}) must be less than or equal to end_date ({})",
                    start_date, end_date
                )
            })),
        )
            .into_response();
    }

    // 6. Aggregate Entity Mentions
    // Grouping map: (entity_text, entity_type) -> (scores, dates)
    let raw_mentions = get_mock_entity_mentions();
    let mut groups: HashMap<(&'static str, &'static str), (Vec<f64>, Vec<NaiveDate>)> =
        HashMap::new();

    for mention in &raw_mentions {
        // Parse date for comparison
        let mention_date = match NaiveDate::parse_from_str(mention.date, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Date range filter
        if mention_date < start_date || mention_date > end_date {
            continue;
        }

        // Entity type filter
        if entity_type != "all" && mention.entity_type != entity_type {
            continue;
        }

        let entry = groups
            .entry((mention.entity_text, mention.entity_type))
            .or_insert_with(|| (Vec::new(), Vec::new()));
        entry.0.push(mention.sentiment_score);
        entry.1.push(mention_date);
    }

    // 7. Calculate Aggregations
    let mut aggregated_entities: Vec<EntitySentimentItem> = Vec::new();

    for ((text, e_type), (scores, dates)) in groups {
        let count = scores.len();
        if count < min_mentions {
            continue;
        }

        let sum_sentiment: f64 = scores.iter().sum();
        let avg_sentiment = (sum_sentiment / count as f64 * 10000.0).round() / 10000.0;

        let pos_count = scores.iter().filter(|&&s| s > 0.1).count();
        let neg_count = scores.iter().filter(|&&s| s < -0.1).count();

        let positive_ratio = (pos_count as f64 / count as f64 * 10000.0).round() / 10000.0;
        let negative_ratio = (neg_count as f64 / count as f64 * 10000.0).round() / 10000.0;

        let latest_date = dates
            .iter()
            .max()
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "".to_string());

        aggregated_entities.push(EntitySentimentItem {
            entity_text: text.to_string(),
            entity_type: e_type.to_string(),
            avg_sentiment,
            positive_ratio,
            negative_ratio,
            mention_count: count,
            latest_mention_date: latest_date,
            model_version: model_version.clone(),
            pipeline_version: pipeline_version.clone(),
            data_provenance: data_provenance.clone(),
        });
    }

    // 8. Sort Results
    match sort_by.as_str() {
        "avg_sentiment" => {
            // Sort by absolute magnitude of avg_sentiment descending, tie-break by mentions descending
            aggregated_entities.sort_by(|a, b| {
                b.avg_sentiment
                    .abs()
                    .partial_cmp(&a.avg_sentiment.abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.mention_count.cmp(&a.mention_count))
            });
        }
        "mentions" => {
            // Sort by mention_count descending, tie-break by absolute sentiment descending
            aggregated_entities.sort_by(|a, b| {
                b.mention_count.cmp(&a.mention_count).then_with(|| {
                    b.avg_sentiment
                        .abs()
                        .partial_cmp(&a.avg_sentiment.abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
        }
        "positive_ratio" => {
            aggregated_entities.sort_by(|a, b| {
                b.positive_ratio
                    .partial_cmp(&a.positive_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.mention_count.cmp(&a.mention_count))
            });
        }
        "negative_ratio" => {
            aggregated_entities.sort_by(|a, b| {
                b.negative_ratio
                    .partial_cmp(&a.negative_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.mention_count.cmp(&a.mention_count))
            });
        }
        _ => {}
    }

    // 9. Apply Limit
    aggregated_entities.truncate(limit);

    let total_count = aggregated_entities.len();

    let response = EntitySentimentResponse {
        start_date: start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        entity_type,
        min_mentions,
        count: total_count,
        entities: aggregated_entities,
        generated_at: Utc::now().to_rfc3339(),
        model_version,
        pipeline_version,
        data_provenance,
    };

    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_default_entity_sentiment_breakdown() {
        let params = SentimentEntitiesParams {
            start_date: Some("2026-08-20".to_string()),
            end_date: Some("2026-08-30".to_string()),
            min_mentions: Some(5),
            entity_type: None,
            limit: Some(20),
            sort_by: None,
        };

        let response =
            get_sentiment_entities_handler(State(AppState::default()), Query(params)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_filter_by_entity_type() {
        let params = SentimentEntitiesParams {
            start_date: Some("2026-08-20".to_string()),
            end_date: Some("2026-08-30".to_string()),
            min_mentions: Some(5),
            entity_type: Some("person".to_string()),
            limit: Some(10),
            sort_by: Some("mentions".to_string()),
        };

        let response =
            get_sentiment_entities_handler(State(AppState::default()), Query(params)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_min_mentions_filtering() {
        let params = SentimentEntitiesParams {
            start_date: Some("2026-08-20".to_string()),
            end_date: Some("2026-08-30".to_string()),
            min_mentions: Some(10),
            entity_type: None,
            limit: Some(20),
            sort_by: None,
        };

        let response =
            get_sentiment_entities_handler(State(AppState::default()), Query(params)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_validation_errors() {
        // limit = 0
        let res_bad_limit = get_sentiment_entities_handler(
            State(AppState::default()),
            Query(SentimentEntitiesParams {
                start_date: None,
                end_date: None,
                min_mentions: None,
                entity_type: None,
                limit: Some(0),
                sort_by: None,
            }),
        )
        .await;
        assert_eq!(res_bad_limit.status(), StatusCode::BAD_REQUEST);

        // min_mentions = 0
        let res_bad_mentions = get_sentiment_entities_handler(
            State(AppState::default()),
            Query(SentimentEntitiesParams {
                start_date: None,
                end_date: None,
                min_mentions: Some(0),
                entity_type: None,
                limit: None,
                sort_by: None,
            }),
        )
        .await;
        assert_eq!(res_bad_mentions.status(), StatusCode::BAD_REQUEST);

        // invalid entity_type
        let res_bad_type = get_sentiment_entities_handler(
            State(AppState::default()),
            Query(SentimentEntitiesParams {
                start_date: None,
                end_date: None,
                min_mentions: None,
                entity_type: Some("invalid_type".to_string()),
                limit: None,
                sort_by: None,
            }),
        )
        .await;
        assert_eq!(res_bad_type.status(), StatusCode::BAD_REQUEST);

        // invalid sort_by
        let res_bad_sort = get_sentiment_entities_handler(
            State(AppState::default()),
            Query(SentimentEntitiesParams {
                start_date: None,
                end_date: None,
                min_mentions: None,
                entity_type: None,
                limit: None,
                sort_by: Some("invalid_sort".to_string()),
            }),
        )
        .await;
        assert_eq!(res_bad_sort.status(), StatusCode::BAD_REQUEST);

        // start_date > end_date
        let res_bad_dates = get_sentiment_entities_handler(
            State(AppState::default()),
            Query(SentimentEntitiesParams {
                start_date: Some("2026-08-30".to_string()),
                end_date: Some("2026-08-20".to_string()),
                min_mentions: None,
                entity_type: None,
                limit: None,
                sort_by: None,
            }),
        )
        .await;
        assert_eq!(res_bad_dates.status(), StatusCode::BAD_REQUEST);
    }
}
