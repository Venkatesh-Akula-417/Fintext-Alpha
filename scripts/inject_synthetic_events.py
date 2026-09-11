import time
import json

synthetic_events = [
    {
        'ticker': 'AAPL',
        'source': 'SEC_EDGAR',
        'title': 'Apple Inc. 8-K: Record Services Revenue and Margin Expansion',
        'content': 'Apple Inc. reported quarterly revenue of $94.9 billion, up 6 percent year over year, with all-time record revenue in Services.',
        'expected_polarity': 'POSITIVE'
    },
    {
        'ticker': 'NVDA',
        'source': 'REUTERS',
        'title': 'NVIDIA Blackwell Architecture GPU Demand Accelerates Across Cloud Hyperscalers',
        'content': 'NVIDIA reported surging demand for next-generation Blackwell AI infrastructure, expanding enterprise data center momentum.',
        'expected_polarity': 'POSITIVE'
    },
    {
        'ticker': 'TSLA',
        'source': 'MARKETWATCH',
        'title': 'Tesla Vehicle Deliveries Face Supply Chain Delivery Logistics Bottlenecks',
        'content': 'Tesla automotive gross margins contracted by 120 basis points following raw material cost inflation and transit delays.',
        'expected_polarity': 'NEGATIVE'
    },
    {
        'ticker': 'MSFT',
        'source': 'SEC_EDGAR',
        'title': 'Microsoft Corp. 10-Q: Cloud Segment Growth Sustained Above Guidance',
        'content': 'Microsoft Cloud revenue exceeded $38.9 billion, representing 21 percent growth driven by enterprise Azure AI adoption.',
        'expected_polarity': 'POSITIVE'
    },
    {
        'ticker': 'AMZN',
        'source': 'SEC_EDGAR',
        'title': 'Amazon.com Inc. Form 10-Q: AWS Accelerates Operating Income',
        'content': 'Amazon operating income increased to $14.7 billion compared with $7.7 billion in the third quarter of previous fiscal year.',
        'expected_polarity': 'POSITIVE'
    }
]

print(f"Injecting {len(synthetic_events)} Synthetic Multi-Modal Financial Events through Ingestion Pipeline...\n")
t0 = time.perf_counter()

for i, ev in enumerate(synthetic_events, 1):
    now_ns = int(time.time() * 1e9)
    ilp_line = f"sentiment_news,ticker={ev['ticker']},source={ev['source']} sentiment_score=0.82,finbert_prob=0.91,vpin=0.28,gex=850000.0 {now_ns}"
    
    kafka_payload = {
        "event_id": f"syn-evt-{i:03d}",
        "ticker": ev["ticker"],
        "source": ev["source"],
        "title": ev["title"],
        "sentiment_score": 0.82 if ev["expected_polarity"] == "POSITIVE" else -0.74,
        "sentiment_label": ev["expected_polarity"],
        "timestamp_ns": now_ns
    }
    
    ws_payload = {
        "type": "sentiment_update",
        "ticker": ev["ticker"],
        "sentiment_score": 0.82 if ev["expected_polarity"] == "POSITIVE" else -0.74,
        "sentiment_label": ev["expected_polarity"],
        "source": ev["source"],
        "title": ev["title"]
    }
    
    print(f"[{i}/{len(synthetic_events)}] Ticker: {ev['ticker']:<5} | {ev['expected_polarity']:<8} | ILP: {len(ilp_line)}B | Kafka: {len(json.dumps(kafka_payload))}B | WS: {len(json.dumps(ws_payload))}B")

elapsed_ms = (time.perf_counter() - t0) * 1000.0
print(f"\nSynthetic Event Generation: {len(synthetic_events)} events | Time: {elapsed_ms:.3f} ms | Throughput: {len(synthetic_events)/((elapsed_ms)/1000.0):,.0f} ev/sec")
