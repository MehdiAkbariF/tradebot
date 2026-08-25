import json
import os
import asyncio
import numpy as np
import redis.asyncio as aioredis
import lightgbm as lgb
from datetime import datetime, timezone
from collections import deque
from loguru import logger

REDIS_URL = "redis://127.0.0.1:6379"

class RealtimeScalpBridge:
    def __init__(self, model_path: str = "scalp_lightgbm_model.txt"):
        self.model = None
        if os.path.exists(model_path):
            self.model = lgb.Booster(model_file=model_path)
            logger.info(f"Loaded LightGBM Scalp Model from {model_path}")
        else:
            logger.warning(f"Model file {model_path} not found. Running with baseline rules.")

        # بافرهای درون‌حافظه‌ای جهت محاسبه فیچرهای زنده بدون تاخیر
        self.price_history = deque(maxlen=3600)   # ۱ ساعت گذشته (برای EMA 15m)
        self.volume_history = deque(maxlen=3600)
        self.active_news = deque(maxlen=50)       # اخبار ۱۰ دقیقه اخیر
        self.decay_lambda = 0.0069
        self.cum_vol = 0.0
        self.cum_vol_price = 0.0

    def get_decayed_sentiment(self) -> float:
        now = datetime.now(timezone.utc)
        total = 0.0
        for n in self.active_news:
            delta = (now - n["ts"]).total_seconds()
            if delta <= 600:
                decay = np.exp(-self.decay_lambda * delta)
                total += n["score"] * decay
        return float(np.clip(total, -1.0, 1.0))

    def compute_features(self, current_price: float, current_vol: float, ofi: float) -> list[float]:
        self.price_history.append(current_price)
        self.volume_history.append(current_vol)
        
        self.cum_vol += current_vol
        self.cum_vol_price += (current_price * current_vol)
        daily_vwap = self.cum_vol_price / self.cum_vol if self.cum_vol > 0 else current_price

        prices = list(self.price_history)
        
        # ۱. بازده‌های ثانیه‌ای
        p_now = prices[-1]
        p_1s = prices[-2] if len(prices) >= 2 else p_now
        p_5s = prices[-6] if len(prices) >= 6 else p_now
        p_15s = prices[-16] if len(prices) >= 16 else p_now

        log_ret_1s = float(np.log(p_now / p_1s)) if p_1s > 0 else 0.0
        log_ret_5s = float(np.log(p_now / p_5s)) if p_5s > 0 else 0.0
        log_ret_15s = float(np.log(p_now / p_15s)) if p_15s > 0 else 0.0

        # ۲. نوسان ۳۰ ثانیه اخیر
        sub_30 = prices[-30:] if len(prices) >= 30 else prices
        rets_30 = np.diff(np.log(sub_30)) if len(sub_30) > 1 else [0.0]
        realized_vol_30s = float(np.std(rets_30)) if len(rets_30) > 1 else 0.0001

        # ۳. روند ۱۵ دقیقه‌ای
        ema_fast = float(pd.Series(prices).ewm(span=900).mean().iloc[-1])
        ema_slow = float(pd.Series(prices).ewm(span=3600).mean().iloc[-1])
        trend_15m_bias = 1.0 if ema_fast > ema_slow else -1.0

        # ۴. فاصله از VWAP
        dist_to_vwap_bps = ((p_now - daily_vwap) / daily_vwap) * 10000.0

        # ۵. جهش حجم
        vol_list = list(self.volume_history)[-60:]
        mean_vol = float(np.mean(vol_list)) if vol_list else 1.0
        volume_surge = current_vol / mean_vol if mean_vol > 0 else 1.0

        # ۶. سنتیمنت لحظه‌ای خبر
        decayed_sentiment = self.get_decayed_sentiment()

        return [
            log_ret_1s, log_ret_5s, log_ret_15s,
            realized_vol_30s, trend_15m_bias,
            dist_to_vwap_bps, volume_surge, ofi,
            decayed_sentiment
        ]

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe("market:trades:btcusdt", "market:metrics:btcusdt")

        logger.info("Realtime Scalp Inference Engine listening to market data & news streams...")
        last_news_id = "0-0"

        while True:
            # ۱. خواندن تیک‌ها و معیارهای اردربوک از Redis
            msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.01)
            if msg:
                channel = msg["channel"]
                try:
                    data = json.loads(msg["data"])
                    if "trades" in channel:
                        price = float(data.get("price", 0.0))
                        vol = float(data.get("quantity", 0.0))
                        ofi = 0.0  # مقدار تیک پیش‌فرض
                        features = self.compute_features(price, vol, ofi)
                        
                        if self.model:
                            prob_up = float(self.model.predict([features])[0])
                            
                            # اگر مدل احتمال بالاتر از ۶۵٪ برای رشد در ۴۵ ثانیه آینده داد
                            if prob_up >= 0.65:
                                signal = {
                                    "symbol": "BTCUSDT",
                                    "action": "BUY",
                                    "probability": round(prob_up, 4),
                                    "trend_bias": features[4],
                                    "decayed_sentiment": features[8],
                                    "timestamp": datetime.now(timezone.utc).isoformat()
                                }
                                await client.publish("market:scalp_signals", json.dumps(signal))
                                logger.info(f"⚡ HIGH CONVICTION SCALP SIGNAL: {signal}")
                except Exception as e:
                    logger.error(f"Error processing market message: {e}")

            # ۲. خواندن غیرهمگام اخبار از Redis Stream و ثبت در بافر سنتیمنت
            news_streams = await client.xread({"events:news_raw": last_news_id}, count=2, block=10)
            if news_streams:
                for _, messages in news_streams:
                    for msg_id, fields in messages:
                        last_news_id = msg_id
                        payload = json.loads(fields.get("payload", "{}"))
                        title = payload.get("title", "")
                        
                        # ارزیابی سریع سنتیمنت خبر (مثبت/منفی)
                        score = 0.8 if any(w in title.lower() for w in ["approve", "bull", "surge", "etf", "record"]) else -0.8 if any(w in title.lower() for w in ["ban", "hack", "drop", "sec", "lawsuit"]) else 0.0
                        
                        self.active_news.append({
                            "ts": datetime.now(timezone.utc),
                            "score": score,
                            "title": title
                        })
                        logger.info(f"📰 Ingested Real-Time News Event | Score: {score} | Title: {title[:45]}")

            await asyncio.sleep(0.001)

if __name__ == "__main__":
    bridge = RealtimeScalpBridge()
    asyncio.run(bridge.run())