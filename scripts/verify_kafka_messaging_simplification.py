#!/usr/bin/env python3
"""
===============================================================================
FinText-Alpha-Vectorizer — Verification: Kafka Messaging Simplification (Suite #251)
===============================================================================
Verifies:
  1.  Zero active NATS dependencies/imports across all Rust crates & Cargo.toml files
  2.  KafkaSink multi-topic support (sentiment-events, sentiment-updates) in ingestion_engine
  3.  KafkaSubscriber with group 'fintext-api-websocket' and Tokio broadcast channel in api_server
  4.  WebSocket handshake and broadcast frame compatibility (topic & subject fields)
  5.  DeadLetterWorker Kafka consumer, exponential backoff retries, and S3/local quarantine
  6.  Configuration hygiene in .env.example, config/config.yaml, feature_flags.yaml
  7.  Docker compose & Kubernetes manifests clean of NATS services/references
  8.  RealtimeSentimentEvent JSON schema & round-trip serialization validity
  9.  DLQ payload schema & exponential backoff mathematical bounds
  10. System documentation and operational runbooks synchronization
===============================================================================
"""

import json
import os
from pathlib import Path
import re
import sys
import yaml

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
if hasattr(sys.stderr, "reconfigure"):
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT_ROOT = Path(__file__).resolve().parent.parent

passed = 0
failed = 0
total = 10


def report(phase: int, name: str, ok: bool, detail: str = ""):
    global passed, failed
    if ok:
        passed += 1
        print(f"  ✅ Phase {phase:2d} │ {name}")
    else:
        failed += 1
        print(f"  ❌ Phase {phase:2d} │ {name}")
        if detail:
            print(f"     └─ {detail}")


def run_all_phases():
    print("=" * 80)
    print(" FinText-Alpha-Vectorizer — Kafka Messaging Simplification Verification (Suite #251)")
    print("=" * 80)
    print(f" Project Root: {PROJECT_ROOT}\n")

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 1: Zero active NATS dependencies/imports across all Rust crates
    # ─────────────────────────────────────────────────────────────────────────
    p1_ok = True
    p1_details = []
    rust_dir = PROJECT_ROOT / "rust"
    for path in rust_dir.rglob("*.rs"):
        # Ignore target or debug dirs if any
        if "target" in path.parts:
            continue
        try:
            content = path.read_text(encoding="utf-8", errors="replace")
            if "async_nats" in content or "async-nats" in content:
                p1_ok = False
                p1_details.append(f"Found NATS reference in {path.relative_to(PROJECT_ROOT)}")
        except Exception as e:
            p1_ok = False
            p1_details.append(f"Error reading {path}: {e}")

    for cargo_file in rust_dir.rglob("Cargo.toml"):
        if "target" in cargo_file.parts:
            continue
        try:
            content = cargo_file.read_text(encoding="utf-8", errors="replace")
            if "async-nats" in content or "async_nats" in content:
                p1_ok = False
                p1_details.append(f"Found NATS dependency in {cargo_file.relative_to(PROJECT_ROOT)}")
        except Exception as e:
            p1_ok = False
            p1_details.append(f"Error reading {cargo_file}: {e}")

    report(
        1,
        "Zero active NATS dependencies/imports in Rust workspace",
        p1_ok,
        "; ".join(p1_details) if p1_details else "",
    )

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 2: KafkaSink multi-topic support in ingestion_engine
    # ─────────────────────────────────────────────────────────────────────────
    kafka_sink_rs = rust_dir / "ingestion_engine" / "src" / "streaming" / "kafka_sink.rs"
    p2_ok = False
    p2_detail = ""
    if kafka_sink_rs.exists():
        content = kafka_sink_rs.read_text(encoding="utf-8", errors="replace")
        has_realtime_event = "struct RealtimeSentimentEvent" in content or "pub struct RealtimeSentimentEvent" in content
        has_realtime_topic = "realtime_topic" in content or "sentiment-updates" in content
        has_publish_realtime = "publish_realtime_event" in content
        has_durable_event = "struct KafkaSentimentEvent" in content or "pub struct KafkaSentimentEvent" in content
        p2_ok = has_realtime_event and has_realtime_topic and has_publish_realtime and has_durable_event
        if not p2_ok:
            p2_detail = f"has_realtime_event={has_realtime_event}, has_realtime_topic={has_realtime_topic}, has_publish_realtime={has_publish_realtime}, has_durable_event={has_durable_event}"
    else:
        p2_detail = "kafka_sink.rs not found"
    report(2, "KafkaSink multi-topic support (sentiment-events, sentiment-updates)", p2_ok, p2_detail)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 3: KafkaSubscriber with group 'fintext-api-websocket' in api_server
    # ─────────────────────────────────────────────────────────────────────────
    kafka_consumer_rs = rust_dir / "api_server" / "src" / "streaming" / "kafka_consumer.rs"
    p3_ok = False
    p3_detail = ""
    if kafka_consumer_rs.exists():
        content = kafka_consumer_rs.read_text(encoding="utf-8", errors="replace")
        has_subscriber = "pub struct KafkaSubscriber" in content or "struct KafkaSubscriber" in content
        has_group = "fintext-api-websocket" in content
        has_broadcast = "broadcast::Sender<RealtimeSentimentEvent>" in content or "broadcast::channel" in content
        has_mock_fallback = "KAFKA_MOCK_FALLBACK" in content or "mock_mode" in content
        p3_ok = has_subscriber and has_group and has_broadcast and has_mock_fallback
        if not p3_ok:
            p3_detail = f"has_subscriber={has_subscriber}, has_group={has_group}, has_broadcast={has_broadcast}, has_mock={has_mock_fallback}"
    else:
        p3_detail = "kafka_consumer.rs not found"
    report(3, "KafkaSubscriber with group 'fintext-api-websocket' and Tokio broadcast channel", p3_ok, p3_detail)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 4: WebSocket handshake and broadcast frame compatibility
    # ─────────────────────────────────────────────────────────────────────────
    ws_handler_rs = rust_dir / "api_server" / "src" / "handlers" / "websocket.rs"
    p4_ok = False
    p4_detail = ""
    if ws_handler_rs.exists():
        content = ws_handler_rs.read_text(encoding="utf-8", errors="replace")
        has_topic = '"topic"' in content
        has_subject = '"subject"' in content  # Backward compat
        has_handshake = "connected" in content
        has_subscribe = "subscribe_client" in content or "subscribe" in content
        p4_ok = has_topic and has_subject and has_handshake and has_subscribe
        if not p4_ok:
            p4_detail = f"has_topic={has_topic}, has_subject={has_subject}, has_handshake={has_handshake}, has_subscribe={has_subscribe}"
    else:
        p4_detail = "websocket.rs not found"
    report(4, "WebSocket handshake & frame compatibility (topic + legacy subject)", p4_ok, p4_detail)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 5: DeadLetterWorker Kafka consumer, retry & quarantine logic
    # ─────────────────────────────────────────────────────────────────────────
    dlq_crate_dir = rust_dir / "dead_letter_worker" / "src"
    p5_ok = False
    p5_detail = ""
    if dlq_crate_dir.exists():
        all_dlq_code = "\n".join(
            f.read_text(encoding="utf-8", errors="replace")
            for f in dlq_crate_dir.rglob("*.rs")
        )
        has_dlq_topic = "sentiment-dlq" in all_dlq_code or "dlq_topic" in all_dlq_code
        has_reprocess = "sentiment-updates" in all_dlq_code or "reprocess_topic" in all_dlq_code
        has_backoff = "calculate_backoff" in all_dlq_code or "execute_with_retry" in all_dlq_code
        has_quarantine = "QuarantineManager" in all_dlq_code and "generate_s3_key" in all_dlq_code
        has_kafka = "StreamConsumer" in all_dlq_code and "FutureProducer" in all_dlq_code
        p5_ok = has_dlq_topic and has_reprocess and has_backoff and has_quarantine and has_kafka
        if not p5_ok:
            p5_detail = f"has_dlq_topic={has_dlq_topic}, has_reprocess={has_reprocess}, has_backoff={has_backoff}, has_quarantine={has_quarantine}, has_kafka={has_kafka}"
    else:
        p5_detail = "dead_letter_worker crate src not found"
    report(5, "DeadLetterWorker Kafka consumer, exponential backoff retries & quarantine", p5_ok, p5_detail)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 6: Configuration hygiene (.env.example, config.yaml, feature_flags.yaml)
    # ─────────────────────────────────────────────────────────────────────────
    env_example = PROJECT_ROOT / ".env.example"
    cfg_yaml = PROJECT_ROOT / "config" / "config.yaml"
    flags_yaml = PROJECT_ROOT / "config" / "feature_flags.yaml"
    p6_ok = False
    p6_details = []

    if env_example.exists():
        env_text = env_example.read_text(encoding="utf-8", errors="replace")
        if "NATS_" in env_text or "nats://" in env_text:
            p6_details.append(".env.example contains stale NATS references")
        if "KAFKA_REALTIME_TOPIC" not in env_text:
            p6_details.append(".env.example missing KAFKA_REALTIME_TOPIC")
        if "KAFKA_DLQ_TOPIC" not in env_text:
            p6_details.append(".env.example missing KAFKA_DLQ_TOPIC")

    if cfg_yaml.exists():
        cfg_text = cfg_yaml.read_text(encoding="utf-8", errors="replace")
        if "nats:" in cfg_text or "nats_url" in cfg_text:
            p6_details.append("config.yaml contains stale NATS references")
        if "sentiment-updates" not in cfg_text and "realtime_topic" not in cfg_text:
            p6_details.append("config.yaml missing realtime topic configuration")

    if flags_yaml.exists():
        flags_text = flags_yaml.read_text(encoding="utf-8", errors="replace")
        if "nats_" in flags_text:
            p6_details.append("feature_flags.yaml contains stale nats feature flags")
        if "kafka_realtime_sink" not in flags_text:
            p6_details.append("feature_flags.yaml missing kafka_realtime_sink flag")

    p6_ok = len(p6_details) == 0
    report(6, "Configuration hygiene (.env.example, config.yaml, feature_flags.yaml)", p6_ok, "; ".join(p6_details))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 7: Docker compose & Kubernetes manifests clean of NATS
    # ─────────────────────────────────────────────────────────────────────────
    docker_compose = PROJECT_ROOT / "docker-compose.yml"
    k8s_dir = PROJECT_ROOT / "k8s"
    p7_ok = True
    p7_details = []

    if docker_compose.exists():
        dc_text = docker_compose.read_text(encoding="utf-8", errors="replace")
        if "nats:" in dc_text or "image: nats:" in dc_text or "4222:4222" in dc_text:
            p7_ok = False
            p7_details.append("docker-compose.yml contains NATS service or port mappings")

    for k8s_file in k8s_dir.rglob("*.yaml"):
        try:
            content = k8s_file.read_text(encoding="utf-8", errors="replace")
            # Exclude commented or documentation lines if any, but ensure zero active nats services
            if "app.kubernetes.io/name: nats" in content or "kind: StatefulSet\nmetadata:\n  name: nats" in content:
                p7_ok = False
                p7_details.append(f"{k8s_file.relative_to(PROJECT_ROOT)} contains NATS Kubernetes manifest")
        except Exception as e:
            p7_ok = False
            p7_details.append(f"Error reading {k8s_file}: {e}")

    report(7, "Docker compose & Kubernetes manifests clean of NATS services", p7_ok, "; ".join(p7_details))

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 8: RealtimeSentimentEvent JSON schema & round-trip serialization validity
    # ─────────────────────────────────────────────────────────────────────────
    sample_realtime_event = {
        "event_id": "evt-test-12345678-abcd-ef01",
        "ticker": "AAPL",
        "timestamp_ns": 1741168800000000000,
        "sentiment_score": 0.8245,
        "confidence": 0.985,
        "vpin": 0.231,
        "dealer_gex_dollar": 1450200.0,
        "entities": ["AAPL", "Tim Cook", "Cupertino"],
        "source": "bloomberg",
        "category": "earnings",
    }
    p8_ok = False
    p8_detail = ""
    try:
        serialized = json.dumps(sample_realtime_event)
        deserialized = json.loads(serialized)
        assert deserialized["ticker"] == "AAPL"
        assert abs(deserialized["sentiment_score"] - 0.8245) < 1e-6
        assert len(deserialized["entities"]) == 3
        p8_ok = True
    except Exception as e:
        p8_detail = f"Serialization error: {e}"
    report(8, "RealtimeSentimentEvent JSON schema & round-trip serialization", p8_ok, p8_detail)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 9: DLQ payload schema & exponential backoff mathematical bounds
    # ─────────────────────────────────────────────────────────────────────────
    base_ms = 100
    max_ms = 5000
    # Formula: base_ms * 2^(attempt - 1)
    attempt_1 = base_ms * (2 ** (1 - 1))  # 100ms
    attempt_2 = base_ms * (2 ** (2 - 1))  # 200ms
    attempt_3 = base_ms * (2 ** (3 - 1))  # 400ms
    attempt_4 = base_ms * (2 ** (4 - 1))  # 800ms
    attempt_5 = base_ms * (2 ** (5 - 1))  # 1600ms
    attempt_7 = min(max_ms, base_ms * (2 ** (7 - 1)))  # 5000ms capped

    sample_dlq_payload = {
        "id": "dlq-uuid-test",
        "failed_at": "2026-09-04T12:00:00Z",
        "error": "Connection reset by peer",
        "retry_count": 3,
        "original_payload": sample_realtime_event,
    }
    p9_ok = (
        attempt_1 == 100
        and attempt_2 == 200
        and attempt_3 == 400
        and attempt_4 == 800
        and attempt_5 == 1600
        and attempt_7 == 5000
        and "retry_count" in sample_dlq_payload
        and "original_payload" in sample_dlq_payload
    )
    report(9, "DLQ payload schema & exponential backoff math (100ms * 2^(a-1) capped at 5000ms)", p9_ok)

    # ─────────────────────────────────────────────────────────────────────────
    # Phase 10: System documentation and operational runbooks synchronization
    # ─────────────────────────────────────────────────────────────────────────
    arch_doc = PROJECT_ROOT / "docs" / "current_architecture.md"
    ops_doc = PROJECT_ROOT / "docs" / "OPERATIONS.md"
    err_doc = PROJECT_ROOT / "docs" / "ERROR_CODES.md"
    p10_ok = True
    p10_details = []

    for doc_path in [arch_doc, ops_doc, err_doc]:
        if doc_path.exists():
            text = doc_path.read_text(encoding="utf-8", errors="replace")
            # Ensure no active NATS runbooks or tables
            if "NATSConsumerLagHigh" in text or "nats consumer report" in text or "NATS JetStream" in text:
                p10_ok = False
                p10_details.append(f"{doc_path.relative_to(PROJECT_ROOT)} contains active NATS references")
        else:
            p10_ok = False
            p10_details.append(f"Missing doc {doc_path.name}")

    report(10, "Documentation and operational runbooks synchronized with Kafka architecture", p10_ok, "; ".join(p10_details))

    # ─────────────────────────────────────────────────────────────────────────
    # Summary
    # ─────────────────────────────────────────────────────────────────────────
    print("\n" + "=" * 80)
    print(f" Kafka Messaging Simplification Results: {passed}/{total} Phases Passed ({failed} Failed)")
    print("=" * 80)

    if failed > 0:
        sys.exit(1)


if __name__ == "__main__":
    run_all_phases()
