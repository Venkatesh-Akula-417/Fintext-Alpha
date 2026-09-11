//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Native Rust Event Taxonomy Classifier
//! ═══════════════════════════════════════════════════════════════════════════════
//!
//! Ultra-high-throughput compiled regex event classifier for algorithmic trading.
//! Eliminates Python GIL contention during mid-frequency news event tagging.
//! ═══════════════════════════════════════════════════════════════════════════════

use once_cell::sync::Lazy;
use pyo3::prelude::*;
use regex::Regex;

struct CategoryPattern {
    category: &'static str,
    patterns: Vec<Regex>,
}

static COMPILED_PATTERNS: Lazy<Vec<CategoryPattern>> = Lazy::new(|| {
    let raw_defs: Vec<(&'static str, Vec<&'static str>)> = vec![
        (
            "M&A",
            vec![
                r"(?i)\bacquires?\b",
                r"(?i)\bacquisition\b",
                r"(?i)\bmergers?\b",
                r"(?i)\bmerged\b",
                r"(?i)\bto merge\b",
                r"(?i)\btakeovers?\b",
                r"(?i)\bbuyouts?\b",
                r"(?i)\bto purchase\b",
                r"(?i)\bpurchases?\b",
                r"(?i)\bpurchased\b",
                r"(?i)\bdeal to buy\b",
                r"(?i)\bbid to buy\b",
            ],
        ),
        (
            "FDA_Approval",
            vec![
                r"(?i)\bfda approves?\b",
                r"(?i)\bfda approved\b",
                r"(?i)\bfda approval\b",
                r"(?i)\bapproved by fda\b",
                r"(?i)\bfda clearance\b",
                r"(?i)\bcleared by fda\b",
                r"(?i)\bfda greenlights?\b",
                r"(?i)\bfda grants approval\b",
                r"(?i)\bfda fast track\b",
            ],
        ),
        (
            "FDA_Rejection",
            vec![
                r"(?i)\bfda rejects?\b",
                r"(?i)\bfda rejected\b",
                r"(?i)\bfda rejection\b",
                r"(?i)\bfda denies?\b",
                r"(?i)\bfda denied\b",
                r"(?i)\bfda declines?\b",
                r"(?i)\bfda declined\b",
                r"(?i)\bcomplete response letter\b",
                r"(?i)\bfda issues crl\b",
                r"(?i)\bfda clinical hold\b",
            ],
        ),
        (
            "CEO_Change",
            vec![
                r"(?i)\bceo resigns?\b",
                r"(?i)\bceo resigned\b",
                r"(?i)\bceo steps down\b",
                r"(?i)\bceo stepped down\b",
                r"(?i)\bnew ceo\b",
                r"(?i)\bceo appointment\b",
                r"(?i)\bappointed as ceo\b",
                r"(?i)\bnames new ceo\b",
                r"(?i)\bceo transition\b",
                r"(?i)\bceo ousted\b",
                r"(?i)\binterim ceo\b",
                r"(?i)\bdeparts as ceo\b",
            ],
        ),
        (
            "Earnings_Beat",
            vec![
                r"(?i)\bbeats? earnings\b",
                r"(?i)\bearnings beat\b",
                r"(?i)\bbeats? estimates\b",
                r"(?i)\brevenue beats?\b",
                r"(?i)\bbeats? expectations\b",
                r"(?i)\btops? estimates\b",
                r"(?i)\btops? expectations\b",
                r"(?i)\bearnings surge\b",
                r"(?i)\beps beats?\b",
                r"(?i)\bprofit beats?\b",
            ],
        ),
        (
            "Earnings_Miss",
            vec![
                r"(?i)\bmisses? earnings\b",
                r"(?i)\bearnings miss\b",
                r"(?i)\bmisses? estimates\b",
                r"(?i)\brevenue miss\b",
                r"(?i)\bmisses? expectations\b",
                r"(?i)\bfalls? short of estimates\b",
                r"(?i)\beps miss\b",
                r"(?i)\bprofit misses?\b",
                r"(?i)\bearnings slump\b",
            ],
        ),
        (
            "Product_Launch",
            vec![
                r"(?i)\blaunches? new\b",
                r"(?i)\blaunched new\b",
                r"(?i)\bproduct launch\b",
                r"(?i)\bunveils?\b",
                r"(?i)\bunveiled\b",
                r"(?i)\bunveiling\b",
                r"(?i)\bintroduces? new\b",
                r"(?i)\bintroduced new\b",
                r"(?i)\bdebuts? new\b",
                r"(?i)\brolls? out new\b",
                r"(?i)\breleases? new product\b",
                r"(?i)\breveals? new\b",
                r"(?i)\bcompany launches\b",
            ],
        ),
        (
            "Regulatory_Action",
            vec![
                r"(?i)\bsec fines?\b",
                r"(?i)\bregulators? fines?\b",
                r"(?i)\bfined by regulators\b",
                r"(?i)\bantitrust\b",
                r"(?i)\bregulatory action\b",
                r"(?i)\bsec investigation\b",
                r"(?i)\bdoj probe\b",
                r"(?i)\bregulatory probe\b",
                r"(?i)\bsec charges\b",
                r"(?i)\bftc lawsuit\b",
                r"(?i)\bsubpoena\b",
            ],
        ),
        (
            "Class_Action_Lawsuit",
            vec![
                r"(?i)\bclass action\b",
                r"(?i)\bclass-action\b",
                r"(?i)\bshareholder lawsuit\b",
                r"(?i)\bsues\b",
                r"(?i)\bsued\b",
                r"(?i)\bfiling lawsuit\b",
                r"(?i)\bfiles? lawsuit\b",
                r"(?i)\bsecurities fraud lawsuit\b",
                r"(?i)\blead plaintiff\b",
            ],
        ),
        (
            "Bankruptcy",
            vec![
                r"(?i)\bbankruptcy\b",
                r"(?i)\bchapter 11\b",
                r"(?i)\bchapter 7\b",
                r"(?i)\binsolvency\b",
                r"(?i)\binsolvent\b",
                r"(?i)\bfiles? for bankruptcy\b",
                r"(?i)\bfiling for bankruptcy\b",
                r"(?i)\breceivership\b",
                r"(?i)\bdefaults? on debt\b",
                r"(?i)\bliquidation\b",
            ],
        ),
        (
            "Dividend_Change",
            vec![
                r"(?i)\bdividend increase\b",
                r"(?i)\bdividend cut\b",
                r"(?i)\braises? dividend\b",
                r"(?i)\braised dividend\b",
                r"(?i)\bdividend hike\b",
                r"(?i)\bcuts? dividend\b",
                r"(?i)\bsuspends? dividend\b",
                r"(?i)\bsuspended dividend\b",
                r"(?i)\bquarterly dividend\b",
                r"(?i)\bspecial dividend\b",
                r"(?i)\bdeclares? dividend\b",
            ],
        ),
        (
            "Stock_Buyback",
            vec![
                r"(?i)\bshare buyback\b",
                r"(?i)\bstock buyback\b",
                r"(?i)\brepurchase program\b",
                r"(?i)\bbuyback program\b",
                r"(?i)\bauthorizes? buyback\b",
                r"(?i)\bannounces? buyback\b",
                r"(?i)\bshares? repurchase\b",
                r"(?i)\brepurchases? shares\b",
            ],
        ),
        (
            "Analyst_Upgrade",
            vec![
                r"(?i)\bupgrades?\b",
                r"(?i)\bupgraded\b",
                r"(?i)\bupgraded to buy\b",
                r"(?i)\bupgrade to buy\b",
                r"(?i)\braises? rating\b",
                r"(?i)\braised rating\b",
                r"(?i)\bupgrade to outperform\b",
                r"(?i)\bupgraded to overweight\b",
                r"(?i)\braises? price target\b",
                r"(?i)\bprice target raised\b",
                r"(?i)\banalysts? upgrade\b",
            ],
        ),
        (
            "Analyst_Downgrade",
            vec![
                r"(?i)\bdowngrades?\b",
                r"(?i)\bdowngraded\b",
                r"(?i)\bdowngraded to sell\b",
                r"(?i)\bdowngrade to sell\b",
                r"(?i)\bcuts? rating\b",
                r"(?i)\bcut rating\b",
                r"(?i)\bdowngrade to underperform\b",
                r"(?i)\bdowngraded to underweight\b",
                r"(?i)\bcuts? price target\b",
                r"(?i)\bprice target lowered\b",
                r"(?i)\banalysts? downgrade\b",
            ],
        ),
        (
            "Insider_Trading",
            vec![
                r"(?i)\binsider trading\b",
                r"(?i)\binsider sells?\b",
                r"(?i)\binsider sold\b",
                r"(?i)\binsider buys?\b",
                r"(?i)\binsider bought\b",
                r"(?i)\bsec filing shows insider\b",
                r"(?i)\bform 4 filing\b",
                r"(?i)\binsider purchase\b",
                r"(?i)\binsider transaction\b",
            ],
        ),
        (
            "Macro_News",
            vec![
                r"(?i)\bfed announces?\b",
                r"(?i)\bfed hike\b",
                r"(?i)\bfed rate\b",
                r"(?i)\binterest rate decision\b",
                r"(?i)\bcpi data\b",
                r"(?i)\bgdp growth\b",
                r"(?i)\bunemployment rate\b",
                r"(?i)\binflation data\b",
                r"(?i)\bfederal reserve\b",
                r"(?i)\bfomc decision\b",
                r"(?i)\bfomc statement\b",
                r"(?i)\bnonfarm payrolls\b",
                r"(?i)\btreasury yields\b",
            ],
        ),
    ];

    raw_defs
        .into_iter()
        .map(|(cat, patterns)| CategoryPattern {
            category: cat,
            patterns: patterns
                .into_iter()
                .filter_map(|p| Regex::new(p).ok())
                .collect(),
        })
        .collect()
});

/// Pure Rust core classifier logic.
fn classify_internal(text: &str) -> String {
    let clean = text.trim();
    if clean.is_empty() {
        return "Other".to_string();
    }

    let mut best_category = "Other";
    let mut best_score: usize = 0;

    for cat_def in COMPILED_PATTERNS.iter() {
        let mut score: usize = 0;
        for pattern in &cat_def.patterns {
            score += pattern.find_iter(clean).count();
        }

        if score > best_score {
            best_score = score;
            best_category = cat_def.category;
        }
    }

    best_category.to_string()
}

/// Standalone module function accessible from Python.
#[pyfunction]
pub fn classify_event(text: &str) -> String {
    classify_internal(text)
}

/// PyO3 Class wrapping the native event classifier.
#[derive(Default)]
#[pyclass]
pub struct RustEventClassifier;

#[pymethods]
impl RustEventClassifier {
    #[new]
    pub fn new() -> Self {
        RustEventClassifier
    }

    /// Classify a single text string.
    pub fn classify(&self, text: &str) -> String {
        classify_internal(text)
    }

    /// Classify a batch of text strings without GIL re-acquisition.
    pub fn classify_batch(&self, texts: Vec<String>) -> Vec<String> {
        texts.into_iter().map(|t| classify_internal(&t)).collect()
    }
}

/// PyO3 Python Module Definition
#[pymodule]
fn fintext_event_classifier(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(classify_event, m)?)?;
    m.add_class::<RustEventClassifier>()?;
    Ok(())
}
