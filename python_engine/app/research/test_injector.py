# مسیر: python_engine/app/research/test_injector.py
import redis
import json
from datetime import datetime, timezone

r = redis.Redis.from_url("redis://127.0.0.1:6379")

def trigger_test_event():
    print("Injecting High-Impact News...")
    news = {
        "id": "news_inject_101",
        "source_id": "Reuters Breaking",
        "title": "Fed Announces Emergency Rate Cut and Approves Direct Institutional Crypto Liquidity",
        "url": "https://reuters.com",
        "timestamp": datetime.now(timezone.utc).isoformat()
    }
    r.xadd("events:news_raw", {"payload": json.dumps(news)})

    print("Publishing Simulated High-Conviction Scalp Signal...")
    signal = {
        "symbol": "BTCUSDT",
        "action": "BUY",
        "probability": 0.895,
        "trend_bias": 1.0,
        "decayed_sentiment": 0.80,
        "timestamp": datetime.now(timezone.utc).isoformat()
    }
    r.publish("market:scalp_signals", json.dumps(signal))

    print("Publishing Simulated Position Exit with Realized PnL...")
    pos = {
        "symbol": "BTCUSDT",
        "action": "CLOSE",
        "entry_price": 65000.0,
        "exit_price": 65145.0,
        "pnl": 145.0,
        "reason": "TAKE_PROFIT_TRIGGERED"
    }
    r.publish("market:positions", json.dumps(pos))
    print("✅ All test events injected into Redis successfully!")

if __name__ == "__main__":
    trigger_test_event()