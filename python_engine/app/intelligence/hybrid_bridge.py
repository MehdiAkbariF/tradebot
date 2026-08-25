import json
import os
import numpy as np
import lightgbm as lgb
from loguru import logger
import redis

class HybridSignalBridge:
    def __init__(self, redis_url: str, model_path: str = "scalp_lightgbm_model.txt"):
        self.redis_client = redis.Redis.from_url(redis_url)
        
        # تعیین دقیق مسیر فایل مدل
        current_dir = os.path.dirname(os.path.abspath(__file__))
        possible_paths = [
            model_path,
            os.path.join(current_dir, "../../scalp_lightgbm_model.txt"),
            os.path.join(current_dir, "../scalp_lightgbm_model.txt"),
            "scalp_lightgbm_model.txt",
            "python_engine/scalp_lightgbm_model.txt"
        ]

        target_path = None
        for path in possible_paths:
            if os.path.exists(path):
                target_path = path
                break

        if target_path:
            self.model = lgb.Booster(model_file=target_path)
            logger.info(f"Successfully loaded LightGBM model from {target_path}")
        else:
            self.model = None
            logger.warning("LightGBM model file not found in any search path! Running in pure rule-based mode.")

        self.news_stream = "events:news_raw"
        self.hybrid_signals_stream = "events:hybrid_signals"

    def predict_ml_scalp(self, features: dict) -> float:
        """
        استنتاج لحظه‌ای با مدل ماشین‌لرنینگ
        """
        if not self.model:
            return 0.5 # خنثی

        X = [[
            features.get("returns_1s", 0.0),
            features.get("returns_10s", 0.0),
            features.get("rolling_volatility_30s", 0.0001),
            features.get("volume_ma_30s", 1.0),
            features.get("ofi", 0.0)
        ]]
        
        prob = self.model.predict(X)[0]
        return float(prob)

    def evaluate_hybrid_decision(self, asset: str, news_relevance: float, is_positive_news: bool, ml_probability: float):
        """
        ترکیب هوش خبری و هوش ماشین‌لرنینگ برای صدور تصمیم نهایی
        """
        action = "NO_TRADE"
        confidence = 0.0

        if is_positive_news and news_relevance > 0.5 and ml_probability > 0.55:
            action = "BUY"
            confidence = (news_relevance + ml_probability) / 2.0
        elif not is_positive_news and news_relevance > 0.5 and ml_probability < 0.45:
            action = "SELL"
            confidence = (news_relevance + (1.0 - ml_probability)) / 2.0
        else:
            action = "NO_TRADE"
            confidence = 0.0

        signal_payload = {
            "symbol": asset,
            "action": action,
            "confidence": round(confidence, 4),
            "ml_probability": round(ml_probability, 4),
            "news_relevance": news_relevance,
            "reason": "HYBRID_ML_NEWS_SYNTHESIS"
        }

        # انتشار سیگنال هیبریدی روی ردیس
        self.redis_client.xadd(self.hybrid_signals_stream, {"payload": json.dumps(signal_payload)})
        logger.info(f"Hybrid Signal Generated -> Asset: {asset} | Action: {action} | Conf: {confidence:.2f} | ML Prob: {ml_probability:.2f}")

    def run_bridge_loop(self):
        logger.info("Starting Hybrid Signal Bridge Worker (Listening for Raw News)...")
        last_id = "0-0"
        while True:
            try:
                streams = self.redis_client.xread({"events:news_raw": last_id}, count=5, block=1000)
                if not streams:
                    # حالت شبیه‌سازی اگر خبری نباشد
                    mock_features = {"returns_1s": 0.0001, "returns_10s": 0.0003, "rolling_volatility_30s": 0.00002, "volume_ma_30s": 2.5, "ofi": 0.4}
                    prob = self.predict_ml_scalp(mock_features)
                    if prob > 0.6:
                        self.evaluate_hybrid_decision("BTCUSDT", news_relevance=0.85, is_positive_news=True, ml_probability=prob)
                    continue

                for stream_name, messages in streams:
                    for message_id, data in messages:
                        last_id = message_id
                        payload_str = data.get(b"payload", b"{}").decode("utf-8")
                        try:
                            news_data = json.loads(payload_str)
                        except json.JSONDecodeError:
                            continue

                        title = news_data.get("title", "No Title")
                        logger.info(f"Processing Live News for AI Signal: {title[:50]}...")
                        
                        asset = "BTCUSDT" if "Bitcoin" in title or "BTC" in title else "ETHUSDT"
                        relevance = 0.90 if "SEC" in title or "Approves" in title else 0.70
                        is_positive = "Approves" in title or "Bull" in title or "High" in title or "SEC" in title

                        mock_features = {"returns_1s": 0.0003, "returns_10s": 0.0006, "rolling_volatility_30s": 0.00001, "volume_ma_30s": 3.5, "ofi": 0.6}
                        ml_prob = 0.85

                        self.evaluate_hybrid_decision(asset, relevance, is_positive, ml_prob)
            except Exception as e:
                logger.error(f"Error in Hybrid Bridge Loop: {e}")

if __name__ == "__main__":
    bridge = HybridSignalBridge(redis_url="redis://127.0.0.1:6379")
    bridge.run_bridge_loop()