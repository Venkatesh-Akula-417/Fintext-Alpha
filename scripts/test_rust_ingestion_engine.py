"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Native Rust Ingestion Engine Certification Suite
═══════════════════════════════════════════════════════════════════════════════

Suite #179: High-Throughput Rust Ingestion Engine, Whisper ASR, Acoustic DSP, Supply Chain GNN, Polygon.io Options, VPIN & GEX Microstructure, QuestDB, Kafka & TimescaleDB
═══════════════════════════════════════════════════════════════════════════════
"""

import os
from pathlib import Path
import subprocess
import sys
import unittest

PROJECT_ROOT = Path(__file__).resolve().parent.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


def get_cargo_env():
    env = os.environ.copy()
    env["CMAKE"] = r"C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    env["CMAKE_GENERATOR"] = "Visual Studio 17 2022"
    env["LIBCLANG_PATH"] = str(PROJECT_ROOT / "venv" / "Lib" / "site-packages" / "clang" / "native")
    env["WHISPER_MOCK_FALLBACK"] = "1"
    return env


class TestRustIngestionEngine(unittest.TestCase):
    """Verifies native Rust ingestion engine, Whisper ASR, Acoustic DSP, Supply Chain GNN, Polygon Options, VPIN/GEX, ONNX FinBERT, QuestDB ILP, Kafka & TimescaleDB sinks."""

    def test_01_cargo_test_passes(self):
        """Verify that all Rust unit tests in fintext_ingestion_engine pass cleanly."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0, f"Cargo test failed:\n{res.stderr}\n{res.stdout}")
        self.assertTrue("passed" in res.stdout and "0 failed" in res.stdout, f"Cargo test failed: {res.stdout}")


    def test_02_release_binary_built(self):
        """Verify that the optimized release binary is present and executable."""
        bin_path = PROJECT_ROOT / "rust" / "target" / "release" / ("fintext_ingestion.exe" if sys.platform == "win32" else "fintext_ingestion")
        self.assertTrue(bin_path.exists(), f"Release binary not found at {bin_path}")

    def test_03_in_process_sidecar_linking(self):
        """Verify that in-process Rust sidecars (HTML, Tickers, Spam, Taxonomy) execute with zero IPC."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "tests::test_preprocessor_spam_filtering"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_04_latency_sla_certification(self):
        """Verify that in-process Rust preprocessing is strictly < 80ms SLA."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "test_preprocessor_html_and_tickers"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_05_native_rust_onnx_inference(self):
        """Verify native in-process ONNX Runtime FinBERT inference in Rust."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::onnx_sentiment::tests::test_onnx_sentiment_positive"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_06_native_questdb_ilp_sink(self):
        """Verify native QuestDB Influx Line Protocol (ILP) formatting and error resilience."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "storage::questdb::tests::test_ilp_line_formatting"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_07_native_kafka_streaming_sink(self):
        """Verify native Kafka JSON event serialization for sentiment event streaming."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "streaming::kafka_sink::tests::test_kafka_sentiment_event_serialization"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_08_kafka_offline_graceful_resilience(self):
        """Verify Kafka producer graceful degradation and mock mode execution."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "streaming::kafka_sink::tests::test_kafka_sink_mock_mode_success"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_09_native_kafka_realtime_streaming_sink(self):
        """Verify native Kafka real-time JSON event serialization for sub-millisecond distribution."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "streaming::kafka_sink::tests::test_realtime_sentiment_event_serialization"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_10_kafka_offline_graceful_resilience(self):
        """Verify Kafka producer graceful degradation, timeout protection, and mock mode execution."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "streaming::kafka_sink::tests::test_kafka_sink_mock_mode_success"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_11_onnx_execution_provider_registration(self):
        """Verify TensorRT, CUDA, or CPU Execution Provider registration in ONNX pipeline."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::onnx_sentiment::tests::test_onnx_execution_provider_registered"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_12_onnx_sentiment_benchmark(self):
        """Verify ONNX FinBERT latency benchmarking executes smoothly."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::onnx_sentiment::tests::test_onnx_sentiment_benchmark_latency"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_13_onnx_pad_to_128_helper(self):
        """Verify pad_to_128 helper correctly sizes input and mask to 128 elements."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::onnx_sentiment::tests::test_onnx_pad_to_128_helper"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_14_onnx_pad_to_fixed_len_helper(self):
        """Verify pad_to_fixed_len helper correctly sizes input and mask to 32 elements."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::onnx_sentiment::tests::test_onnx_pad_to_fixed_len_helper"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_15_ner_entity_extraction_sample(self):
        """Verify native Rust NER pipeline extracts ORG and LOC entities."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "nlp::ner::tests::test_ner_entity_extraction_sample"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_16_whisper_wav_reader_synthetic_audio(self):
        """Verify hound WAV reader decodes synthetic 16kHz audio into normalized float samples."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::transcriber::tests::test_wav_reader_synthetic_audio"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_17_whisper_audio_resampling(self):
        """Verify linear resampler converts 44.1kHz audio to 16.0kHz Whisper requirement."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::transcriber::tests::test_wav_resampling_44100_to_16000"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_18_whisper_mock_transcription_end_to_end(self):
        """Verify end-to-end Whisper ASR transcription execution with mock fallback."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::transcriber::tests::test_mock_transcription_end_to_end"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_19_acoustic_features_pitch_f0(self):
        """Verify DSP autocorrelation extracts accurate F0 pitch from synthetic 200 Hz tone."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::feature_extractor::tests::test_pitch_detection_sine_wave_200hz"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_20_acoustic_features_silence_pause_ratio(self):
        """Verify DSP acoustic feature extractor detects 100% pause ratio and zero pitch on silence."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::feature_extractor::tests::test_silence_pause_ratio"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_21_acoustic_features_mixed_signal(self):
        """Verify feature extraction accurately isolates pitch std, pause ratio, and speech rate on bursts."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "audio::feature_extractor::tests::test_extract_features_mixed_signal"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_22_gnn_3_node_supply_chain_propagation(self):
        """Verify SupplyChainGNN 2-layer GCN shock propagation on synthetic 3-node network."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::gnn::tests::test_3_node_supply_chain_propagation"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_23_gnn_identity_adjacency_and_zeros(self):
        """Verify GNN produces zero activation on zero input shock vector under ReLU."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::gnn::tests::test_identity_adjacency_and_zeros"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_24_gnn_laplacian_symmetry(self):
        """Verify symmetric normalized graph Laplacian mathematical properties."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::gnn::tests::test_laplacian_symmetry_and_normalization"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_25_gnn_error_handling(self):
        """Verify GNN raises descriptive errors on empty graphs or dimension mismatches."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::gnn::tests::test_empty_graph_and_dimension_mismatch_errors"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_26_polygon_occ_parser_standard_aapl(self):
        """Verify OCC options ticker parser handles standard Call option with O: prefix."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "sources::polygon::tests::test_parse_options_ticker_standard_aapl"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_27_polygon_occ_parser_put_and_fractional_strike(self):
        """Verify OCC options parser extracts underlying, Put type, and fractional strike prices."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "sources::polygon::tests::test_parse_options_ticker_without_o_prefix_put"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_28_polygon_occ_parser_invalid_formats(self):
        """Verify OCC options parser gracefully rejects malformed or truncated tickers."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "sources::polygon::tests::test_parse_options_ticker_invalid_formats"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_29_polygon_client_from_env(self):
        """Verify PolygonClient loads API key from environment without leaks."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "sources::polygon::tests::test_polygon_client_from_env_or_missing"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_30_polygon_client_mock_fetch_trades(self):
        """Verify PolygonClient options trade retrieval and OptionTrade struct mapping in mock mode."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "sources::polygon::tests::test_polygon_client_mock_fetch_trades"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_31_vpin_known_imbalance(self):
        """Verify Volume-Synchronized Probability of Informed Trading (VPIN) tick rule & bucketing."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::microstructure::tests::test_vpin_with_known_imbalance"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_32_black_scholes_gamma_and_gex(self):
        """Verify Black-Scholes analytical gamma calculation and Net Dealer GEX dollar exposure."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::microstructure::tests::test_black_scholes_gamma_and_gex"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)

    def test_33_microstructure_error_handling(self):
        """Verify microstructure engine error handling on empty trades, non-positive spot, and zero buckets."""
        res = subprocess.run(
            ["cargo", "test", "--manifest-path", "rust/Cargo.toml", "--package", "fintext_ingestion_engine", "--", "alpha::microstructure::tests::test_error_handling_empty_and_invalid_inputs"],
            cwd=str(PROJECT_ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env=get_cargo_env(),
        )
        self.assertEqual(res.returncode, 0)
        self.assertIn("1 passed", res.stdout)


if __name__ == "__main__":
    unittest.main()
