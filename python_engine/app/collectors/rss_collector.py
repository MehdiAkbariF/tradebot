# مسیر: python_engine/app/collectors/rss_collector.py
import hashlib
import json
import time
from datetime import datetime, timezone
import feedparser
from loguru import logger
from pydantic import BaseModel
import redis

class TimeAudit(BaseModel):
    source_ts: datetime
    received_ts: datetime
    processed_ts: datetime
    available_ts: datetime

class RawNewsMessage(BaseModel):
    id: str
    source_id: str
    external_id: str
    title: str
    body: str | None = None
    url: str | None = None
    time_audit: TimeAudit
    hash_signature: str

class RSSCollector:
    def __init__(self, redis_url: str, feeds: list[str]):
        self.redis_client = redis.Redis.from_url(redis_url)
        self.feeds = feeds
        self.seen_hashes = set()

    def generate_hash(self, text: str) -> str:
        return hashlib.sha256(text.encode("utf-8")).hexdigest()

    def fetch_feeds(self):
        for feed_url in self.feeds:
            try:
                parsed = feedparser.parse(feed_url)
                received_ts = datetime.now(timezone.utc)

                for entry in parsed.entries:
                    title = entry.get("title", "")
                    summary = entry.get("summary", entry.get("description", ""))
                    link = entry.get("link", "")
                    guid = entry.get("id", link)

                    if not title:
                        continue

                    content_to_hash = f"{title}_{guid}"
                    hash_sig = self.generate_hash(content_to_hash)

                    if hash_sig in self.seen_hashes:
                        continue
                    self.seen_hashes.add(hash_sig)

                    pub_time = entry.get("published_parsed") or entry.get("updated_parsed")
                    if pub_time:
                        source_ts = datetime.fromtimestamp(time.mktime(pub_time), tz=timezone.utc)
                    else:
                        source_ts = received_ts

                    processed_ts = datetime.now(timezone.utc)

                    news_msg = RawNewsMessage(
                        id=hash_sig[:16],
                        source_id=feed_url,
                        external_id=guid,
                        title=title,
                        body=summary,
                        url=link,
                        time_audit=TimeAudit(
                            source_ts=source_ts,
                            received_ts=received_ts,
                            processed_ts=processed_ts,
                            available_ts=processed_ts,
                        ),
                        hash_signature=hash_sig,
                    )

                    payload_json = news_msg.model_dump_json()

                    # ۱. ذخیره در استریم
                    self.redis_client.xadd("events:news_raw", {"payload": payload_json})
                    # ۲. انتشار همزمان روی کانال Pub/Sub برای ارسال آنی به وب‌سوکت داشبورد
                    self.redis_client.publish("events:news_raw", payload_json)
                    logger.info(f"📰 Published News: {title[:50]}...")

            except Exception as e:
                logger.error(f"Error fetching feed {feed_url}: {e}")

    def run_loop(self, interval_sec: int = 30):
        logger.info("Starting RSS News Collector Loop...")
        while True:
            self.fetch_feeds()
            time.sleep(interval_sec)

if __name__ == "__main__":
    sample_feeds = [
        "https://www.coindesk.com/arc/outboundfeeds/rss/",
        "https://cointelegraph.com/rss",
    ]
    collector = RSSCollector(redis_url="redis://127.0.0.1:6379", feeds=sample_feeds)
    collector.run_loop(interval_sec=30)