import json
import os
import numpy as np
import lightgbm as lgb
from loguru import logger
import redis

class HybridSignalBridge:
    def __init__(self, redis_url: str, model_path: str = "scalp_lightgbm_model.txt"):
        self.redis_client = redis.Redis.from_url(redis_url)
        
        # تعیین دقیق مسیر فایل مدل (جستجو در پوشه جاری و پوشه‌های والد)
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

        self.news_stream = "events:market_events"
        self.hybrid_signals_stream = "events:hybrid_signals"

    def predict_ml_scalp(self, features: dict) -> float:
        """
        استنتاج لحظه‌ای با مدل ماشین‌لرنینگ
        features شامل: returns_1s, returns_10s, rolling_volatility_30s, volume_ma_30s, ofi
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
        logger.info("Starting Hybrid Signal Bridge Worker...")
        while True:
            streams = self.redis_client.xread({self.news_stream: "0-1"}, count=5, block=1000)
            if not streams:
                # شبیه‌سازی یک تیک لایو برای تست مدل ML حتی اگر خبری نیاید
                mock_features = {"returns_1s": 0.0001, "returns_10s": 0.0003, "rolling_volatility_30s": 0.00002, "volume_ma_30s": 2.5, "ofi": 0.4}
                prob = self.predict_ml_scalp(mock_features)
                if prob > 0.6:
                    self.evaluate_hybrid_decision("BTCUSDT", news_relevance=0.8, is_positive_news=True, ml_probability=prob)
                continue

            for stream_name, messages in streams:
                for message_id, data in messages:
                    payload = json.loads(data[b"payload"].decode("utf-8"))
                    asset = payload.get("primary_asset", "BTCUSDT")
                    relevance = payload.get("relevance_score", 0.5)
                    
                    is_positive = True 

                    mock_features = {"returns_1s": 0.0002, "returns_10s": 0.0004, "rolling_volatility_30s": 0.00001, "volume_ma_30s": 3.0, "ofi": 0.5}
                    ml_prob = self.predict_ml_scalp(mock_features)

                    self.evaluate_hybrid_decision(asset, relevance, is_positive, ml_prob)
                    self.redis_client.xdel(self.news_stream, message_id)

if __name__ == "__main__":
    bridge = HybridSignalBridge(redis_url="redis://127.0.0.1:6379")
    bridge.run_bridge_loop()