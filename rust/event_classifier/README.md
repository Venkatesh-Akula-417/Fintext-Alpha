# FinText Corporate Event Taxonomy Classifier (`fintext_event_classifier`)

> **Last Verified**: 2026-09-10 (Suite #96) | **Audit Readiness**: Certified Clean | **Authoritative Deprecations**: [`docs/DEPRECATED.md`](../../docs/DEPRECATED.md)

## 1. Module Name & Purpose
`fintext_event_classifier` is an ultra-high-throughput deterministic corporate event classifier. It categorizes unstructured financial headlines and regulatory filing summaries into a hierarchical corporate event taxonomy (M&A, FDA decisions, earnings beats/misses, executive turnover, dividends, litigation, restructuring, etc.) in sub-millisecond time.

## 2. Architecture
- **Hierarchical Pattern Matching**: Matches incoming text sequentially against precompiled regex patterns in `COMPILED_PATTERNS`.
- **Supported Event Taxonomy**:
  - `M&A`: Acquisitions, mergers, buyouts, takeovers.
  - `FDA_Approval` / `FDA_Rejection`: Drug approvals, CRLs, clinical holds.
  - `CEO_Change`: Resignations, appointments, departures.
  - `Earnings_Beat` / `Earnings_Miss`: EPS, revenue, profit beats or misses.
  - `Dividend_Hike` / `Dividend_Cut`: Dividend increases, cuts, suspensions.
  - `Stock_Buyback`: Share repurchase authorizations.
  - `Litigation_Settlement`: Lawsuit settlements, fines, patent disputes.
  - `Debt_Default` / `Bankruptcy`: Chapter 11 filings, missed interest payments.
  - `Product_Launch`: Key product and service announcements.
  - `General_News`: Fallback baseline for non-catalyst news.

## 3. Dependencies
| Dependency | Version | Purpose |
| :--- | :--- | :--- |
| `regex` | `1.10` | High-performance deterministic finite automaton (DFA) matching |
| `once_cell` | `1.19` | Lazy initialization of compiled regex hierarchy |
| `pyo3` | `0.29` | Optional Python C-extension bindings (`cdylib`) |

## 4. Public API
- `pub fn classify_event(text: &str) -> String`: Returns the primary taxonomy category.
- `pub struct RustEventClassifier`: PyO3 class supporting single and batch document classification without Python GIL overhead.

## 5. Testing
Run crate unit tests:
```bash
cargo test -p fintext_event_classifier
```

## 6. Usage Example
```rust
use fintext_event_classifier::classify_event;

let category = classify_event("Microsoft acquires AI robotics startup for $2.5B in all-cash transaction");
assert_eq!(category, "M&A");

let fda_cat = classify_event("FDA grants fast track designation for oncology therapeutic candidate");
assert_eq!(fda_cat, "FDA_Approval");
```

## 7. Related Modules
- [`fintext_ingestion_engine`](../ingestion_engine): Tags preprocessed documents with event categories for downstream event-study and backtesting engines.
- [`fintext_api_server`](../api_server): Filters and queries event-driven sentiment feeds.
