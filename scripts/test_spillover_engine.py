"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Cross-Asset Spillover Engine Certification Suite
═══════════════════════════════════════════════════════════════════════════════

Suite #181: Cross-Asset Sentiment Spillover, Lead-Lag Analytics, Matrix Alignment & QuestDB ILP
═══════════════════════════════════════════════════════════════════════════════
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import unittest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


class TestSpilloverEngine(unittest.TestCase):
    """Verifies the native Rust cross-asset spillover engine, correlation calculations, hourly alignment, and QuestDB ILP persistence."""

    def test_01_cargo_test_passes(self):
        """Verify that all Rust unit tests in fintext_spillover_engine pass cleanly."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_spillover_engine"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        self.assertEqual(res.returncode, 0, f"Cargo test failed:\n{res.stderr}\n{res.stdout}")
        self.assertIn("13 passed", res.stdout)

    def test_02_release_binary_built(self):
        """Verify that the optimized release binary is present and executable."""
        bin_path = PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_spillover.exe" if sys.platform == "win32" else "fintext_spillover")
        self.assertTrue(bin_path.exists(), f"Release binary not found at {bin_path}")

    def test_03_lead_lag_correlation_unit_test(self):
        """Verify lead-lag shift detection logic across positive and negative hours."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_spillover_engine", "--", "correlation::tests::test_cross_correlation_positive_lag_lead"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_04_synchronized_hourly_matrix_alignment(self):
        """Verify hourly bucketing and timeline alignment across tickers."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_spillover_engine", "--", "timeseries::tests::test_build_synchronized_matrix_basic"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_05_mock_mode_end_to_end_cycle(self):
        """Verify engine run_once end-to-end execution in mock mode."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_spillover_engine", "--", "engine::tests::test_engine_run_once_mock_mode"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)


if __name__ == "__main__":
    unittest.main()
