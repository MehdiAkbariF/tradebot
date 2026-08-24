import json
from datetime import datetime, timezone
from loguru import logger
import redis
from pydantic import BaseModel

class MacroEventMessage(BaseModel):
    event_id: str
    event_name: str
    country: str
    currency: str
    actual: float
    forecast: float
    previous: float
    surprise_z_score: float
    source_ts: str
    available_ts: str

class EventCorrelatorWorker:
    def __init__(self, redis_url: str):
        self.redis_client = redis.Redis.from_url(redis_url)
        self.input_stream = "events:market_events"
        self.macro_output_stream = "events:macro_surprises"
        
    def calculate_economic_surprise(self, actual: float, forecast: float, previous: float) -> float:
        """
        محاسبه شاخص غافلگیری اقتصادی (Economic Surprise Z-Score ساده‌سازی شده)
        فرمول: (Actual - Forecast) / StdDev (فرض انحراف معیار استاندارد بر اساس تفاوت)
        """
        deviation = actual - forecast
        # فرض یک انحراف معیار فرضی برای سنجش شدت غافلگیری
        std_dev = abs(forecast) * 0.1 if forecast != 0 else 0.1
        z_score = deviation / std_dev if std_dev > 0 else 0.0
        return round(z_score, 4)

    def process_macro_simulation(self, event_name: str, actual: float, forecast: float, previous: float):
        """
        شبیه‌سازی دریافت داده کلان اقتصادی و محاسبه Surprise
        """
        z_score = self.calculate_economic_surprise(actual, forecast, previous)
        now_str = datetime.now(timezone.utc).isoformat()

        macro_msg = MacroEventMessage(
            event_id=f"macro_{int(datetime.now().timestamp())}",
            event_name=event_name,
            country="US",
            currency="USD",
            actual=actual,
            forecast=forecast,
            previous=previous,
            surprise_z_score=z_score,
            source_ts=now_str,
            available_ts=now_str
        )

        self.redis_client.xadd(
            self.macro_output_stream,
            {"payload": macro_msg.model_dump_json()}
        )
        logger.info(f"Macro Surprise Calculated -> Event: {event_name} | Actual: {actual} | Forecast: {forecast} | Z-Score: {z_score}")

    def run_correlator(self):
        logger.info("Starting Event Correlator & Surprise Engine...")
        
        # تستی برای شبیه‌سازی یک رویداد کلان اقتصادی (مثلاً Non-Farm Payrolls)
        self.process_macro_simulation("US Non-Farm Payrolls", actual=275.0, forecast=200.0, previous=229.0)

        while True:
            # دریافت رویدادهای بازار از استریم
            streams = self.redis_client.xread({self.input_stream: "0-1"}, count=5, block=2000)
            if not streams:
                continue

            for stream_name, messages in streams:
                for message_id, data in messages:
                    payload = json.loads(data[b"payload"].decode("utf-8"))
                    asset = payload.get("primary_asset")
                    summary = payload.get("summary")
                    
                    # Deduplication & Correlation check (ساده‌سازی: تجمیع بر اساس دارایی در پنجره زمانی)
                    logger.info(f"Correlated Market Event -> Asset: {asset} | Event: {summary[:50]}")
                    
                    # پاکسازی از استریم پردازش‌شده
                    self.redis_client.xdel(self.input_stream, message_id)

if __name__ == "__main__":
    correlator = EventCorrelatorWorker(redis_url="redis://127.0.0.1:6379")
    correlator.run_correlator()