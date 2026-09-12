# FinText Alpha Vectorizer — Point-in-Time (PIT) Correctness Certification Report

> **Overall Status**: ✅ CERTIFIED PASS (PostgreSQL 16 Storage Layer Verified)  
> **Audit Level**: Tier-1 Institutional Quantitative SLA  
> **Evaluation Scope**: SCD Type 2 Temporal Isolation, Symbol Lineage, Restatements, Delistings, Stock Splits, Index Rebalances, and Negative Control Sensitivity.  

---

## 1. Executive Summary

This validation test harness was executed against **PostgreSQL 16.15 + TimescaleDB 2.30.0** on target `localhost:5432/fintext_metadata`. Point-in-Time correctness and zero look-ahead bias have been verified directly against the production relational storage stack. At any historical query point $T$, the platform exposes exclusively the data state observable at $T$, with future revisions, restatements, delayed ingestions, and subsequent index changes completely hidden.

### Key Audit Results
- **Positive As-Of Scenarios (S1–S8)**: 8 / 8 PASSED (100% compliance)
- **Negative Control Sensitivity (N1)**: 1 / 1 VERIFIED (Look-ahead defect detected as EXPECTED_FAILURE)
- **Overall Certification Result**: **PASS**

---

## 2. Test Execution Metadata

| Metric | Value |
| :--- | :--- |
| **Validation Database Engine** | PostgreSQL 16.15 + TimescaleDB 2.30.0 |
| **Git Commit SHA** | `0ecae145d9b047131d569d7480d22707e72ec09f` |
| **Run Started (UTC)** | `2026-09-12T06:43:29Z` |
| **Run Finished (UTC)** | `2026-09-12T06:43:30Z` |
| **Database Target** | `localhost:5432/fintext_metadata` |
| **Test Schema Isolation** | `fintext_pit_validation_test` (ephemeral & isolated) |

### Validation Environment
| Parameter | Setting |
| :--- | :--- |
| **Engine Profile** | PostgreSQL 16.15 + TimescaleDB 2.30.0 |
| **Host Target** | `localhost:5432/fintext_metadata` |
| **Execution Timestamp** | `2026-09-12T06:43:29Z` |
| **Storage Certification Status** | CERTIFIED (Production PostgreSQL 16) |

### One-Line Reproduction Command
```bash
python scripts/validate_pit_correctness.py --database-url "postgres://fintext:fintext@localhost:5432/fintext_metadata" --output-dir data/pit-validation --report-path docs/PIT_VALIDATION_REPORT.md
```

---

## 3. Scenario Matrix Summary

| ID | Scenario Name | As-Of Timestamp | Expected Rows | Actual Rows | Status |
| :--- | :--- | :--- | :---: | :---: | :---: |
| **S1** | Basic as-of before revision (T1 < rev2.valid_from) | `2025-06-15T12:00:00Z` | 1 | 1 | ✅ PASS |
| **S2** | As-of after revision (T2 >= rev2.valid_from) | `2025-06-15T15:00:00Z` | 1 | 1 | ✅ PASS |
| **S3** | Late-arriving event zero look-ahead isolation (Tp < Ta < Ti) | `2025-06-16T10:00:00Z` | 0 | 0 | ✅ PASS |
| **S4** | Restated earnings revision historical transition | `2025-05-05T12:00:00Z, 2025-05-15T12:00:00Z` | 2 | 2 | ✅ PASS |
| **S5** | Ticker change and bi-temporal symbol lineage replay | `2020-01-15T00:00:00Z, 2023-01-15T00:00:00Z` | 4 | 4 | ✅ PASS |
| **S6** | Delisting resolution and terminal return retrieval | `2023-04-15T00:00:00Z, 2023-05-02T00:00:00Z` | 1 | 1 | ✅ PASS |
| **S7** | Corporate action split adjustment factor | `2022-08-20T00:00:00Z, 2022-08-26T00:00:00Z` | 1 | 1 | ✅ PASS |
| **S8** | Index membership point-in-time constituent set (survivorship bias prevention) | `2020-11-01T00:00:00Z, 2021-01-15T00:00:00Z` | 4 | 4 | ✅ PASS |
| **N1** | Negative Control: Naive query look-ahead sensitivity detector | `2025-06-15T12:00:00Z` | 1 | 1 | ⚠️ EXPECTED_FAILURE |

---

## 4. Scenario Deep-Dive & Mathematical Evidence

### Scenario S1: Basic as-of before revision (T1 < rev2.valid_from)
- **Status**: **PASS**
- **As-Of Query Timestamp**: `2025-06-15T12:00:00Z`
- **Expected Row Count**: `1` | **Actual Row Count**: `1`
- **Evidence Details**:
```json
{
  "query_timestamp": "2025-06-15T12:00:00Z",
  "returned_revisions": [
    1
  ],
  "returned_scores": [
    0.85
  ],
  "target_record": {
    "id": 1001,
    "ticker": "AAPL",
    "revision_number": 1,
    "sentiment_score": 0.85,
    "sentiment_label": "POSITIVE",
    "valid_from": "2025-06-15T10:00:00+00:00",
    "valid_to": "2025-06-15T14:00:00+00:00",
    "is_current": false
  },
  "invariant": "At T1=12:00, only rev1 (valid [10:00, 14:00)) is returned. rev2 (valid >= 14:00) is excluded."
}
```

### Scenario S2: As-of after revision (T2 >= rev2.valid_from)
- **Status**: **PASS**
- **As-Of Query Timestamp**: `2025-06-15T15:00:00Z`
- **Expected Row Count**: `1` | **Actual Row Count**: `1`
- **Evidence Details**:
```json
{
  "query_timestamp": "2025-06-15T15:00:00Z",
  "returned_revisions": [
    2
  ],
  "returned_scores": [
    0.45
  ],
  "target_record": {
    "id": 1002,
    "ticker": "AAPL",
    "revision_number": 2,
    "sentiment_score": 0.45,
    "sentiment_label": "NEUTRAL",
    "valid_from": "2025-06-15T14:00:00+00:00",
    "valid_to": null,
    "is_current": true
  },
  "invariant": "At T2=15:00, rev2 is active (is_current=True, valid_to=NULL). rev1 expired at 14:00."
}
```

### Scenario S3: Late-arriving event zero look-ahead isolation (Tp < Ta < Ti)
- **Status**: **PASS**
- **As-Of Query Timestamp**: `2025-06-16T10:00:00Z`
- **Expected Row Count**: `0` | **Actual Row Count**: `0`
- **Evidence Details**:
```json
{
  "query_timestamp": "2025-06-16T10:00:00Z",
  "event_published_utc": "2025-06-16T09:00:00Z",
  "event_ingested_utc": "2025-06-16T11:00:00Z",
  "matched_rows": [],
  "invariant": "News published at 09:00 but ingested at 11:00 was unobservable at 10:00. 0 rows returned proves zero look-ahead."
}
```

### Scenario S4: Restated earnings revision historical transition
- **Status**: **PASS**
- **As-Of Query Timestamp**: `['2025-05-05T12:00:00Z', '2025-05-15T12:00:00Z']`
- **Expected Row Count**: `2` | **Actual Row Count**: `2`
- **Evidence Details**:
```json
{
  "t1_query": {
    "as_of": "2025-05-05T12:00:00Z",
    "rev": 1,
    "score": 0.9
  },
  "t3_query": {
    "as_of": "2025-05-15T12:00:00Z",
    "rev": 2,
    "score": 0.3
  },
  "invariant": "Before restatement (T1=05-05), initial filing (score=0.90) is returned. After restatement (T3=05-15), amended filing (score=0.30) is returned."
}
```

### Scenario S5: Ticker change and bi-temporal symbol lineage replay
- **Status**: **PASS**
- **As-Of Query Timestamp**: `['2020-01-15T00:00:00Z', '2023-01-15T00:00:00Z']`
- **Expected Row Count**: `4` | **Actual Row Count**: `4`
- **Evidence Details**:
```json
{
  "t0_ticker": "FB",
  "t3_ticker": "META",
  "complete_lineage": [
    "FB",
    "META"
  ],
  "invariant": "In 2020, entity was 'FB'. In 2023, entity was 'META'. Full lineage correctly links both."
}
```

### Scenario S6: Delisting resolution and terminal return retrieval
- **Status**: **PASS**
- **As-Of Query Timestamp**: `['2023-04-15T00:00:00Z', '2023-05-02T00:00:00Z']`
- **Expected Row Count**: `1` | **Actual Row Count**: `1`
- **Evidence Details**:
```json
{
  "t0_delisted_records": 0,
  "t1_delisted_records": 1,
  "t1_delisting_return": -0.954,
  "t1_reason": "FDIC Receivership & NYSE Delisting",
  "invariant": "Prior to 2023-05-01, FRC was not delisted. After 2023-05-01, delisting return (-95.4%) is captured."
}
```

### Scenario S7: Corporate action split adjustment factor
- **Status**: **PASS**
- **As-Of Query Timestamp**: `['2022-08-20T00:00:00Z', '2022-08-26T00:00:00Z']`
- **Expected Row Count**: `1` | **Actual Row Count**: `1`
- **Evidence Details**:
```json
{
  "unadjusted_price": 900.0,
  "t0_split_factor": 1.0,
  "t0_adjusted_price": 900.0,
  "t1_split_factor": 3.0,
  "t1_adjusted_price": 300.0,
  "invariant": "Before 3:1 split (T0=08-20), adj price is $900. After split (T1=08-26), adj price is $300."
}
```

> [!NOTE]
> **S7 Scope Note**: Note: split adjustment is computed in Python. This scenario verifies corporate_action lookup visibility, not DB-level price adjustment. DB-level price adjustment validation is a separate P1 item.

### Scenario S8: Index membership point-in-time constituent set (survivorship bias prevention)
- **Status**: **PASS**
- **As-Of Query Timestamp**: `['2020-11-01T00:00:00Z', '2021-01-15T00:00:00Z']`
- **Expected Row Count**: `4` | **Actual Row Count**: `4`
- **Evidence Details**:
```json
{
  "t0_constituents": [
    "AAPL",
    "OXY"
  ],
  "t1_constituents": [
    "AAPL",
    "TSLA"
  ],
  "survivorship_check": "TSLA not in S&P500 at T0=2020-11-01; OXY present at T0 and departed by T1."
}
```

### Scenario N1: Negative Control: Naive query look-ahead sensitivity detector
- **Status**: **EXPECTED_FAILURE**
- **As-Of Query Timestamp**: `2025-06-15T12:00:00Z`
- **Expected Row Count**: `1` | **Actual Row Count**: `1`
- **Evidence Details**:
```json
{
  "query_timestamp": "2025-06-15T12:00:00Z",
  "naive_query": "SELECT * FROM sentiment_records WHERE ticker = 'AAPL' AND is_current = TRUE;",
  "expected_revision": 1,
  "actual_revision": 2,
  "lookahead_bias_detected": true,
  "defect_description": "Look-ahead defect caught: Naive query at T1=12:00:00Z returned revision 2 (valid_from=2025-06-15 14:00:00+00:00) instead of historical revision 1.",
  "sensitivity_proof": "Proves that omitting bi-temporal intervals leaks future revisions."
}
```

---

## 5. Negative Control & Test Sensitivity Proof

To rigorously prove that this test harness is genuinely sensitive to look-ahead bias and does not produce false positives, scenario **N1** runs an intentionally broken query:
```sql
SELECT * FROM sentiment_records WHERE ticker = 'AAPL' AND is_current = TRUE;
```
When evaluated at historical timestamp $T_1 = \text{2025-06-15T12:00:00Z}$, this naive query ignores `valid_from` and `valid_to`, incorrectly retrieving revision 2 (which was not published/valid until 14:00:00Z).

The harness detected this defect (`actual_revision = 2 != expected_revision = 1`), successfully marking the negative control as **`EXPECTED_FAILURE`**.
Had the harness failed to detect this leak, the test suite would have exited with a non-zero code.

---

## 6. Continuous Integration (CI) Invocation Guide

To incorporate this Point-in-Time correctness validation into GitHub Actions or automated institutional verification pipelines, add the following step to CI workflows:

```yaml
      - name: Validate Point-in-Time Correctness (Zero Look-Ahead Bias)
        run: |
          python -m pip install -r requirements-validation.txt
          python scripts/validate_pit_correctness.py \
            --database-url "${{ secrets.DATABASE_URL }}" \
            --output-dir data/pit-validation \
            --report-path docs/PIT_VALIDATION_REPORT.md
```

> [!IMPORTANT]
> The CI workflow requires `${{ secrets.DATABASE_URL }}` configured to a live PostgreSQL 16 + TimescaleDB instance. The `sqlite://:memory:` flag is strictly designated as a local development logic fallback and is NOT accepted for institutional audit certification.

### Exit Codes Specification
- **`0`**: Success — All 8 positive scenarios passed and negative control N1 failed as expected.
- **`1`**: Test Failure — Any positive scenario failed OR negative control N1 failed to detect look-ahead bias.
- **`2`**: Environment Error — Database connection refused, invalid credentials, missing drivers, or unset `--database-url`.

---

## 7. Institutional Certification Sign-Off

Certified on PostgreSQL 16 + TimescaleDB: zero look-ahead bias verified at the storage layer.
