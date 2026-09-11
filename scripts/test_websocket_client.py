"""
═══════════════════════════════════════════════════════════════════════════════
FinText-Alpha-Vectorizer — Real-Time WebSocket Test Client
═══════════════════════════════════════════════════════════════════════════════

Connects to the native Rust Axum WebSocket stream at `ws://127.0.0.1:8000/ws`
and prints streaming sentiment updates broadcast from the Kafka sentiment pipeline.
═══════════════════════════════════════════════════════════════════════════════
"""

import asyncio
import json
import sys

try:
    import websockets
except ImportError:
    print("Notice: 'websockets' library is required for this test client.")
    print("Install via: pip install websockets")
    sys.exit(1)


async def listen_sentiment_stream(uri: str = "ws://127.0.0.1:8000/ws"):
    print(f"Connecting to Axum WebSocket Gateway at {uri}...")
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected! Waiting for real-time sentiment signals (Ctrl+C to stop)...\n")
            async for raw_message in websocket:
                try:
                    data = json.loads(raw_message)
                    if data.get("type") == "connected":
                        print(f" Handshake Received: {data}")
                        print("-" * 75)
                    else:
                        ticker = data.get("ticker", "UNKNOWN")
                        score = data.get("sentiment_score", 0.0)
                        label = data.get("sentiment_label", "NEUTRAL")
                        source = data.get("source", "UNKNOWN")
                        title = data.get("title", "")
                        print(f"[{source}] ${ticker} | Score: {score:+.2f} ({label}) | {title[:50]}...")
                except json.JSONDecodeError:
                    print(f" Raw: {raw_message}")
    except Exception as e:
        print(f" Connection failed: {e}")


if __name__ == "__main__":
    url = sys.argv[1] if len(sys.argv) > 1 else "ws://127.0.0.1:8000/ws"
    try:
        asyncio.run(listen_sentiment_stream(url))
    except KeyboardInterrupt:
        print("\nDisconnected from WebSocket stream.")
