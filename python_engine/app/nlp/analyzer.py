import json
from datetime import datetime, timezone
from loguru import logger
import redis
from pydantic import BaseModel

class MarketEventMessage(BaseModel):
    event_id: str
    event_type: str
    primary_asset: str
    relevance_score: float
    novelty_score: float
    summary: str
    first_seen_ts: str
    available_ts: str
    related_news_ids: list[str]

class NewsIntelligenceWorker:
    def __init__(self, redis_url: str):
        self.redis_client = redis.Redis.from_url(redis_url)
        self.stream_input = "events:news_raw"
        self.stream_output = "events:market_events"
        self.asset_keywords = {
            "BTCUSDT": ["bitcoin", "btc", "satoshi"],
            "ETHUSDT": ["ethereum", "eth", "vitalik"],
            "OPUSDT": ["optimism", "op token"],
        }

    def map_asset_and_relevance(self, title: str, body: str) -> tuple[str, float]:
        text = f"{title} {body}".lower()
        for asset, keywords in self.asset_keywords.items():
            for kw in keywords:
                if kw in text:
                    # ساده‌سازی قانون Relevance بر اساس تعداد تکرار یا وجود کلمات کلیدی
                    return asset, 0.85
        return "GENERAL", 0.30

    def process_pending_news(self):
        # خواندن اخبار جدید از Redis Stream
        streams = self.redis_client.xread({self.stream_input: "0-1"}, count=10, block=1000)
        
        if not streams:
            return

        for stream_name, messages in streams:
            for message_id, data in messages:
                payload_str = data.get(b"payload")
                if not payload_str:
                    continue

                news_data = json.loads(payload_str.decode("utf-8"))
                title = news_data.get("title", "")
                body = news_data.get("body", "")
                news_id = news_data.get("id")
                available_ts = news_data["time_audit"]["available_ts"]

                # 1. Entity Extraction & Asset Mapping
                primary_asset, relevance = self.map_asset_and_relevance(title, body)

                # 2. Novelty Scoring (برای MVP فرض می‌کنیم همه اخبار جدید هستند مگر تکرار هک شده باشند)
                novelty = 0.90 if relevance > 0.5 else 0.40

                # 3. ساخت MarketEvent ساختاریافته
                event_msg = MarketEventMessage(
                    event_id=f"evt_{news_id}",
                    event_type="NEWS_MENTION",
                    primary_asset=primary_asset,
                    relevance_score=relevance,
                    novelty_score=novelty,
                    summary=title,
                    first_seen_ts=available_ts,
                    available_ts=available_ts,
                    related_news_ids=[news_id]
                )

                # 4. انتشار روی استریم خروجی رویدادهای بازار
                self.redis_client.xadd(
                    self.stream_output,
                    {"payload": event_msg.model_dump_json()}
                )
                logger.info(f"Intelligence Processed -> Asset: {primary_asset} | Rel: {relevance} | Title: {title[:40]}")

                # Ack / Delete from pending if using consumer groups, for now XACK style simulation
                self.redis_client.xdel(self.stream_input, message_id)

    def run_worker(self):
        logger.info("Starting News Intelligence Worker...")
        while True:
            try:
                self.process_pending_news()
            except Exception as e:
                logger.error(f"Error in Intelligence Worker: {e}")

if __name__ == "__main__":
    worker = NewsIntelligenceWorker(redis_url="redis://127.0.0.1:6379")
    worker.run_worker()