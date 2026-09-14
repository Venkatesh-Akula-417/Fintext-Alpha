//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Multilingual Language Detection & Model Routing Engine
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use std::collections::HashSet;

pub const DEFAULT_ENGLISH_MODEL: &str = "finbert-v3.1.0";
pub const MULTILINGUAL_MODEL_VERSION: &str = "multilingual-minilm-v1.0";
pub const MAX_ANALYSIS_CHARS: usize = 500;

/// Stopword sets for high-precision language classification of financial texts.
static ENGLISH_STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "the",
        "and",
        "is",
        "in",
        "it",
        "to",
        "of",
        "for",
        "with",
        "on",
        "that",
        "this",
        "are",
        "as",
        "at",
        "be",
        "by",
        "from",
        "have",
        "has",
        "market",
        "stock",
        "stocks",
        "shares",
        "growth",
        "revenue",
        "quarter",
        "earnings",
        "bullish",
        "bearish",
        "investors",
        "analysts",
        "report",
        "company",
        "profit",
        "results",
        "year",
        "rate",
    ]
    .iter()
    .copied()
    .collect()
});

static SPANISH_STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "el",
        "la",
        "los",
        "las",
        "un",
        "una",
        "unos",
        "unas",
        "de",
        "en",
        "que",
        "es",
        "por",
        "para",
        "con",
        "del",
        "al",
        "como",
        "mas",
        "más",
        "pero",
        "sus",
        "su",
        "este",
        "esta",
        "estos",
        "estas",
        "mercado",
        "acciones",
        "bolsa",
        "crecimiento",
        "ingresos",
        "ganancias",
        "trimestre",
        "inversores",
        "informe",
        "empresa",
        "beneficio",
        "resultados",
        "año",
        "tasa",
        "precio",
        "bursatil",
        "bursátil",
        "alza",
        "baja",
    ]
    .iter()
    .copied()
    .collect()
});

static GERMAN_STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "der",
        "die",
        "das",
        "und",
        "in",
        "den",
        "von",
        "zu",
        "mit",
        "ist",
        "des",
        "nicht",
        "eine",
        "einer",
        "einem",
        "einen",
        "eines",
        "dem",
        "sich",
        "auf",
        "fuer",
        "für",
        "auch",
        "als",
        "nach",
        "wie",
        "im",
        "am",
        "markt",
        "aktien",
        "aktie",
        "wachstum",
        "umsatz",
        "quartal",
        "gewinn",
        "anleger",
        "bericht",
        "unternehmen",
        "ergebnis",
        "jahr",
        "kurs",
        "boerse",
        "börse",
        "stark",
        "anstieg",
        "rueckgang",
        "rückgang",
    ]
    .iter()
    .copied()
    .collect()
});

static FRENCH_STOPWORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "le",
        "la",
        "les",
        "un",
        "une",
        "des",
        "du",
        "et",
        "est",
        "en",
        "dans",
        "pour",
        "par",
        "sur",
        "qui",
        "avec",
        "tout",
        "plus",
        "au",
        "aux",
        "sont",
        "cette",
        "ces",
        "son",
        "sa",
        "ses",
        "marche",
        "marché",
        "actions",
        "croissance",
        "revenus",
        "benefice",
        "bénéfice",
        "trimestre",
        "investisseurs",
        "rapport",
        "societe",
        "société",
        "resultats",
        "résultats",
        "annee",
        "année",
        "cours",
        "bourse",
        "hausse",
        "baisse",
    ]
    .iter()
    .copied()
    .collect()
});

/// Evaluated result of language detection and model routing.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageDetectionResult {
    /// Canonical language name: "english", "spanish", "german", "french", "japanese"
    pub language: String,
    /// ISO 639-1 language code: "en", "es", "de", "fr", "ja"
    pub language_code: String,
    /// Confidence score between 0.0 and 1.0
    pub confidence: f64,
    /// Total characters analyzed
    pub analyzed_chars: usize,
    /// Whether a multilingual inference model should be applied
    pub is_multilingual_model_applied: bool,
    /// Target model version tag
    pub model_version: String,
}

/// Detects language from the given text (analyzing up to the first 500 characters).
pub fn detect_language(text: &str) -> LanguageDetectionResult {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return LanguageDetectionResult {
            language: "english".to_string(),
            language_code: "en".to_string(),
            confidence: 0.50,
            analyzed_chars: 0,
            is_multilingual_model_applied: false,
            model_version: DEFAULT_ENGLISH_MODEL.to_string(),
        };
    }

    // Limit analysis window to first 500 chars safely across UTF-8 boundaries
    let sample_len = trimmed.len().min(MAX_ANALYSIS_CHARS);
    let sample_slice = match trimmed.char_indices().nth(sample_len) {
        Some((idx, _)) => &trimmed[..idx],
        None => trimmed,
    };
    let analyzed_chars = sample_slice.chars().count();

    // 1. Check for Japanese Unicode scripts (Hiragana, Katakana, CJK Ideographs / Kanji)
    let mut jp_kana_count = 0usize;
    let mut jp_cjk_count = 0usize;
    let mut total_non_whitespace = 0usize;

    for c in sample_slice.chars() {
        if !c.is_whitespace() {
            total_non_whitespace += 1;
        }
        match c {
            // Hiragana
            '\u{3040}'..='\u{309F}' => jp_kana_count += 1,
            // Katakana + Phonetic Extensions
            '\u{30A0}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' => jp_kana_count += 1,
            // CJK Unified Ideographs (Kanji)
            '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' => jp_cjk_count += 1,
            _ => {}
        }
    }

    if total_non_whitespace > 0 {
        if jp_kana_count >= 1
            || (jp_cjk_count >= 2
                && (jp_kana_count + jp_cjk_count) * 10 >= total_non_whitespace * 2)
        {
            let ratio = (jp_kana_count + jp_cjk_count) as f64 / total_non_whitespace as f64;
            let confidence = (0.75 + (ratio * 0.24)).clamp(0.75, 0.99);
            let rounded_conf = (confidence * 100.0).round() / 100.0;
            return LanguageDetectionResult {
                language: "japanese".to_string(),
                language_code: "ja".to_string(),
                confidence: rounded_conf,
                analyzed_chars,
                is_multilingual_model_applied: true,
                model_version: MULTILINGUAL_MODEL_VERSION.to_string(),
            };
        }
    }

    // 2. Tokenize words for Latin-script stopwords comparison
    let mut en_score = 0usize;
    let mut es_score = 0usize;
    let mut de_score = 0usize;
    let mut fr_score = 0usize;
    let mut total_words = 0usize;

    // Clean tokens: lowercase, strip punctuation
    for raw_word in sample_slice.split_whitespace() {
        let clean_word: String = raw_word
            .chars()
            .filter(|c| c.is_alphabetic())
            .flat_map(|c| c.to_lowercase())
            .collect();

        if clean_word.is_empty() {
            continue;
        }
        total_words += 1;

        if ENGLISH_STOPWORDS.contains(clean_word.as_str()) {
            en_score += 1;
        }
        if SPANISH_STOPWORDS.contains(clean_word.as_str()) {
            es_score += 1;
        }
        if GERMAN_STOPWORDS.contains(clean_word.as_str()) {
            de_score += 1;
        }
        if FRENCH_STOPWORDS.contains(clean_word.as_str()) {
            fr_score += 1;
        }
    }

    // 3. Special diacritic hints for tie-breaking Latin scripts
    let lower_sample = sample_slice.to_lowercase();
    if lower_sample.contains('ñ') || lower_sample.contains('¿') || lower_sample.contains('¡') {
        es_score += 2;
    }
    if lower_sample.contains('ä')
        || lower_sample.contains('ö')
        || lower_sample.contains('ü')
        || lower_sample.contains('ß')
    {
        de_score += 2;
    }
    if lower_sample.contains('ç')
        || lower_sample.contains('œ')
        || lower_sample.contains("d'")
        || lower_sample.contains("l'")
        || lower_sample.contains("qu'")
    {
        fr_score += 2;
    }

    let max_score = en_score.max(es_score).max(de_score).max(fr_score);

    if max_score > 0 {
        let total_matches = (en_score + es_score + de_score + fr_score).max(1);
        let dominance_ratio = max_score as f64 / total_matches as f64;
        let word_ratio = (max_score as f64 / total_words.max(1) as f64).min(1.0);
        let confidence = (0.60 + (dominance_ratio * 0.25) + (word_ratio * 0.14)).clamp(0.60, 0.98);
        let rounded_conf = (confidence * 100.0).round() / 100.0;

        if es_score == max_score && es_score > en_score {
            return LanguageDetectionResult {
                language: "spanish".to_string(),
                language_code: "es".to_string(),
                confidence: rounded_conf,
                analyzed_chars,
                is_multilingual_model_applied: true,
                model_version: MULTILINGUAL_MODEL_VERSION.to_string(),
            };
        } else if de_score == max_score && de_score > en_score {
            return LanguageDetectionResult {
                language: "german".to_string(),
                language_code: "de".to_string(),
                confidence: rounded_conf,
                analyzed_chars,
                is_multilingual_model_applied: true,
                model_version: MULTILINGUAL_MODEL_VERSION.to_string(),
            };
        } else if fr_score == max_score && fr_score > en_score {
            return LanguageDetectionResult {
                language: "french".to_string(),
                language_code: "fr".to_string(),
                confidence: rounded_conf,
                analyzed_chars,
                is_multilingual_model_applied: true,
                model_version: MULTILINGUAL_MODEL_VERSION.to_string(),
            };
        } else {
            return LanguageDetectionResult {
                language: "english".to_string(),
                language_code: "en".to_string(),
                confidence: rounded_conf,
                analyzed_chars,
                is_multilingual_model_applied: false,
                model_version: DEFAULT_ENGLISH_MODEL.to_string(),
            };
        }
    }

    // Default fallback: English with baseline confidence
    LanguageDetectionResult {
        language: "english".to_string(),
        language_code: "en".to_string(),
        confidence: 0.50,
        analyzed_chars,
        is_multilingual_model_applied: false,
        model_version: DEFAULT_ENGLISH_MODEL.to_string(),
    }
}

/// Deterministically computes simulated multilingual sentiment scores for non-English records.
pub fn compute_multilingual_sentiment(
    ticker: &str,
    language: &str,
    seed_text: &str,
) -> (f64, String, f32, String) {
    if language == "english" {
        return (
            0.25,
            "BULLISH".to_string(),
            0.85,
            DEFAULT_ENGLISH_MODEL.to_string(),
        );
    }

    // Hash ticker + language + seed_text
    let mut hash = 5381u64;
    for b in ticker
        .bytes()
        .chain(language.bytes())
        .chain(seed_text.bytes())
    {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as u64);
    }

    let score_raw = ((hash % 160) as i64 - 80) as f64 / 100.0; // range [-0.80, 0.79]
    let score = (score_raw * 100.0).round() / 100.0;

    let (label, confidence) = if score >= 0.15 {
        ("BULLISH".to_string(), 0.80 + ((score * 0.18) as f32).abs())
    } else if score <= -0.15 {
        ("BEARISH".to_string(), 0.80 + ((score * 0.18) as f32).abs())
    } else {
        ("NEUTRAL".to_string(), 0.75)
    };

    (
        score,
        label,
        (confidence * 100.0).round() / 100.0,
        MULTILINGUAL_MODEL_VERSION.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english_headline() {
        let text =
            "Apple reports record quarterly revenue and strong iPhone demand in global markets";
        let res = detect_language(text);
        assert_eq!(res.language, "english");
        assert_eq!(res.language_code, "en");
        assert!(res.confidence >= 0.60);
        assert!(!res.is_multilingual_model_applied);
        assert_eq!(res.model_version, DEFAULT_ENGLISH_MODEL);
    }

    #[test]
    fn test_detect_spanish_headline() {
        let text =
            "El mercado de valores muestra un fuerte crecimiento en las acciones de tecnología";
        let res = detect_language(text);
        assert_eq!(res.language, "spanish");
        assert_eq!(res.language_code, "es");
        assert!(res.confidence >= 0.70);
        assert!(res.is_multilingual_model_applied);
        assert_eq!(res.model_version, MULTILINGUAL_MODEL_VERSION);
    }

    #[test]
    fn test_detect_german_headline() {
        let text = "Der DAX verzeichnet ein starkes Wachstum und hohe Gewinne im ersten Quartal";
        let res = detect_language(text);
        assert_eq!(res.language, "german");
        assert_eq!(res.language_code, "de");
        assert!(res.confidence >= 0.70);
        assert!(res.is_multilingual_model_applied);
        assert_eq!(res.model_version, MULTILINGUAL_MODEL_VERSION);
    }

    #[test]
    fn test_detect_french_headline() {
        let text =
            "Le marché boursier enregistre une hausse significative des revenus trimestriels";
        let res = detect_language(text);
        assert_eq!(res.language, "french");
        assert_eq!(res.language_code, "fr");
        assert!(res.confidence >= 0.70);
        assert!(res.is_multilingual_model_applied);
        assert_eq!(res.model_version, MULTILINGUAL_MODEL_VERSION);
    }

    #[test]
    fn test_detect_japanese_headline() {
        let text = "日経平均株価が上昇、テクノロジー銘柄に買いが集まる";
        let res = detect_language(text);
        assert_eq!(res.language, "japanese");
        assert_eq!(res.language_code, "ja");
        assert!(res.confidence >= 0.75);
        assert!(res.is_multilingual_model_applied);
        assert_eq!(res.model_version, MULTILINGUAL_MODEL_VERSION);
    }

    #[test]
    fn test_detect_empty_text() {
        let res = detect_language("");
        assert_eq!(res.language, "english");
        assert_eq!(res.confidence, 0.50);
        assert_eq!(res.analyzed_chars, 0);
    }

    #[test]
    fn test_multilingual_sentiment_computation() {
        let (score, label, conf, model) =
            compute_multilingual_sentiment("AAPL", "spanish", "Crecimiento de ingresos");
        assert_eq!(model, MULTILINGUAL_MODEL_VERSION);
        assert!(score >= -1.0 && score <= 1.0);
        assert!(conf >= 0.70 && conf <= 1.0);
        assert!(label == "BULLISH" || label == "BEARISH" || label == "NEUTRAL");
    }
}
