import redis
import json
from datetime import datetime, timezone

r = redis.Redis.from_url("redis://127.0.0.1:6379")

# شبیه‌سازی یک خبر مهم درباره بیت‌کوین
fake_news = {
    "id": "test_news_999",
    "source_id": "manual_test",
    "external_id": "ext_999",
    "title": "SEC Approves First Spot Bitcoin and Ethereum Hybrid ETF with Instant Stabling",
    "body": "In a historic move, regulators greenlit massive institutional inflows into crypto spot markets.",
    "url": "https://example.com",
    "time_audit": {
        "source_ts": datetime.now(timezone.utc).isoformat(),
        "received_ts": datetime.now(timezone.utc).isoformat(),
        "processed_ts": datetime.now(timezone.utc).isoformat(),
        "available_ts": datetime.now(timezone.utc).isoformat()
    },
    "hash_signature": "abcdef123456"
}

# انتشار روی استریم اخبار خام
r.xadd("events:news_raw", {"payload": json.dumps(fake_news)})
print("Fake high-impact news injected into Redis!")