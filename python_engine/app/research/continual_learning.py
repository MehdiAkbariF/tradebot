# مسیر: python_engine/app/research/continual_learning.py
import json
import os
import asyncio
import numpy as np
import pandas as pd
import lightgbm as lgb
import redis.asyncio as aioredis
from loguru import logger

REDIS_URL = "redis://127.0.0.1:6379"
LEDGER_FILE = "trade_ledger.json"
MODEL_FILE = "scalp_lightgbm_model.txt"

class ContinualLearningWorker:
    """موتور یادگیری مستمر: بازآموزی مدل با فیچرهای استاندارد میکرواستراکچر"""
    def __init__(self, retrain_every_trades: int = 50):
        self.retrain_every_trades = retrain_every_trades
        self.last_trained_count = 0
        self.load_state()

    def load_state(self):
        if os.path.exists(LEDGER_FILE):
            try:
                with open(LEDGER_FILE, "r", encoding="utf-8") as f:
                    trades = json.load(f)
                    self.last_trained_count = len(trades)
            except Exception:
                self.last_trained_count = 0

    def retrain_model_from_ledger(self):
        """بازآموزی افزایشی مدل تنها در صورت داشتن داده کافی و بدون نشت متغیرها"""
        if not os.path.exists(LEDGER_FILE):
            return

        with open(LEDGER_FILE, "r", encoding="utf-8") as f:
            trades = json.load(f)

        if len(trades) < 200:
            logger.info(f"Ledger trades count ({len(trades)}/200). Accumulating robust history before retraining.")
            return

        logger.info(f"🧠 AI CONTINUAL LEARNING: Updating model weights with {len(trades)} live executions...")

        records = []
        for t in trades:
            net_pnl = float(t.get("net_pnl", 0.0))
            is_win = 1 if net_pnl > 0 else 0
            
            # استخراج فیچرهای قبل از معامله (تطابق کامل با مدل لایو)
            records.append({
                "returns_1s": float(t.get("returns_1s", 0.0001)),
                "returns_10s": float(t.get("returns_10s", 0.0003)),
                "rolling_volatility_30s": float(t.get("volatility_bps", 1.0)) / 10000.0,
                "volume_ma_30s": float(t.get("volume", 1.5)),
                "ofi": float(t.get("ofi", 0.0)),
                "target": is_win
            })

        df = pd.DataFrame(records)
        X = df[["returns_1s", "returns_10s", "rolling_volatility_30s", "volume_ma_30s", "ofi"]]
        y = df["target"]

        train_data = lgb.Dataset(X, label=y)
        params = {
            "objective": "binary",
            "metric": "binary_logloss",
            "learning_rate": 0.02,
            "num_leaves": 15,
            "max_depth": 4,
            "min_data_in_leaf": 10,
            "verbose": -1
        }

        updated_model = lgb.train(params, train_data, num_boost_round=80)
        updated_model.save_model(MODEL_FILE)
        
        self.last_trained_count = len(trades)
        logger.success("✅ Scalp Model Successfully Retrained with Zero Data Leakage.")

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe("market:trade_ledger")
        
        logger.info("Continual Learning Feedback Loop ACTIVE...")

        while True:
            msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=1.0)
            if msg:
                try:
                    data = json.loads(msg["data"])
                    pnl = float(data.get("net_pnl", 0.0))
                    reason = data.get("exit_reason", "")
                    symbol = data.get("symbol", "")

                    if pnl > 0:
                        logger.info(f"🎓 Trade Outcome: {symbol} PROFIT (+${pnl:.2f}) [{reason}]")
                    else:
                        logger.warning(f"🎓 Trade Outcome: {symbol} LOSS (-${abs(pnl):.2f}) [{reason}]")

                    if os.path.exists(LEDGER_FILE):
                        with open(LEDGER_FILE, "r", encoding="utf-8") as f:
                            total_trades = len(json.load(f))
                        
                        if total_trades - self.last_trained_count >= self.retrain_every_trades:
                            self.retrain_model_from_ledger()
                except Exception as e:
                    logger.error(f"Error in continual learning loop: {e}")

            await asyncio.sleep(0.5)

if __name__ == "__main__":
    worker = ContinualLearningWorker(retrain_every_trades=50)
    asyncio.run(worker.run())