# مسیر: python_engine/app/intelligence/realtime_bridge.py
import json
import os
import uuid
import asyncio
from datetime import datetime, timezone
from collections import deque
import numpy as np
import pandas as pd
import redis.asyncio as aioredis
from loguru import logger

REDIS_URL = "redis://127.0.0.1:6379"
STRATEGY_VERSION = "wave_breakout_v1"
MODEL_VERSION = "lgb_v2.0"
FEATURE_SCHEMA_VERSION = "2.0.0"

class RealtimeScalpBridge:
    def __init__(self):
        self.price_history = {}
        self.volume_history = {}
        self.buy_vol_history = {}
        self.sell_vol_history = {}
        self.last_signal_time = {}
        self.last_heartbeat_time = 0.0

    def evaluate_live_market(self, symbol: str, price: float, vol: float, is_buyer_maker: bool) -> dict | None:
        now = datetime.now(timezone.utc)
        
        if symbol not in self.price_history:
            self.price_history[symbol] = deque(maxlen=400)
            self.volume_history[symbol] = deque(maxlen=400)
            self.buy_vol_history[symbol] = deque(maxlen=400)
            self.sell_vol_history[symbol] = deque(maxlen=400)
            self.last_signal_time[symbol] = 0.0

        hist_p = self.price_history[symbol]
        hist_v = self.volume_history[symbol]
        hist_bv = self.buy_vol_history[symbol]
        hist_sv = self.sell_vol_history[symbol]
        
        hist_p.append(price)
        hist_v.append(vol)

        if not is_buyer_maker:
            hist_bv.append(vol)
            hist_sv.append(0.0)
        else:
            hist_bv.append(0.0)
            hist_sv.append(vol)

        if len(hist_p) < 60:
            return None

        prices = list(hist_p)
        volumes = list(hist_v)
        p_now = prices[-1]
        p_5s = prices[-6] if len(prices) >= 6 else prices[0]
        p_15s = prices[-16] if len(prices) >= 16 else prices[0]

        sub_30 = prices[-30:]
        volatility_bps = float(((max(sub_30) - min(sub_30)) / sub_30[0]) * 10000.0) if len(sub_30) > 1 else 0.0
        
        now_sec = now.timestamp()

        # فیلتر رِنج (Chop Filter)
        if volatility_bps < 1.20:
            if (now_sec - self.last_heartbeat_time) > 8.0:
                self.last_heartbeat_time = now_sec
                logger.info(f"💤 CHOP FILTER (Market in Range): {symbol} Vol={volatility_bps:.2f} bps | Waiting for expansion (>1.20 bps)...")
            return None
        
        vol_recent_10 = sum(volumes[-10:])
        vol_avg_60 = (sum(volumes[-60:]) / 60.0) * 10.0
        is_volume_expanding = bool(vol_recent_10 >= (vol_avg_60 * 1.4))

        long_window = min(len(prices), 200)
        ma_long = float(np.mean(prices[-long_window:]))

        ofi_window = min(len(hist_bv), 40)
        b_vol = sum(list(hist_bv)[-ofi_window:])
        s_vol = sum(list(hist_sv)[-ofi_window:])
        tot_vol = b_vol + s_vol
        live_ofi = float((b_vol - s_vol) / tot_vol) if tot_vol > 0 else 0.0

        ret_5s = (p_now - p_5s) / p_5s if p_5s > 0 else 0.0
        ret_15s = (p_now - p_15s) / p_15s if p_15s > 0 else 0.0
        price_drift_bps = float((p_now - ma_long) / ma_long * 10000.0)

        alpha_score = (live_ofi * 0.50) + (np.clip(ret_5s * 3000.0, -0.30, 0.30)) + (np.clip(ret_15s * 2000.0, -0.20, 0.20))

        if (now_sec - self.last_heartbeat_time) > 3.0:
            self.last_heartbeat_time = now_sec
            logger.info(f"🌊 WAVE SCAN: {symbol} | Range: {volatility_bps:.2f} bps | OFI: {live_ofi:+.2f} | Alpha: {alpha_score:+.2f}")

        # صدور سیگنال استاندارد و دارای شناسه رهگیری
        if (now_sec - self.last_signal_time[symbol]) > 10.0:
            action = None
            if p_now > p_5s > p_15s and p_now > ma_long and live_ofi > 0.45 and alpha_score >= 0.50 and is_volume_expanding:
                action = "BUY"
            elif p_now < p_5s < p_15s and p_now < ma_long and live_ofi < -0.45 and alpha_score <= -0.50 and is_volume_expanding:
                action = "SELL"

            if action:
                self.last_signal_time[symbol] = now_sec
                prob = round(0.88 + (abs(alpha_score) * 0.10), 4)
                signal_id = str(uuid.uuid4())
                
                canonical_signal = {
                    "signal_id": signal_id,
                    "strategy_version": STRATEGY_VERSION,
                    "model_version": MODEL_VERSION,
                    "feature_schema_version": FEATURE_SCHEMA_VERSION,
                    "symbol": symbol,
                    "action": action,
                    "probability": prob,
                    "alpha_score": round(alpha_score, 4),
                    "ofi": round(live_ofi, 4),
                    "range_bps": round(volatility_bps, 2),
                    "price_drift_bps": round(price_drift_bps, 2),
                    "is_volume_expanding": is_volume_expanding,
                    "signal_price": str(price),
                    "timestamp": now.isoformat()
                }
                return canonical_signal

        return None

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe("market:trades:btcusdt", "market:trades:ethusdt")

        logger.info("🌊 CANONICAL WAVE SCALPER ACTIVE...")

        while True:
            msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.01)
            if msg:
                try:
                    data = json.loads(msg["data"])
                    symbol = data.get("symbol", "").upper()
                    price = float(data.get("price", 0.0))
                    vol = float(data.get("quantity", 0.0))
                    is_buyer_maker = bool(data.get("is_buyer_maker", False))
                    
                    signal = self.evaluate_live_market(symbol, price, vol, is_buyer_maker)
                    if signal:
                        await client.publish("market:scalp_signals", json.dumps(signal))
                        logger.success(f"🚀 SIGNAL PUBLISHED [ID: {signal['signal_id'][:8]}]: {signal['symbol']} -> {signal['action']}")
                except Exception as e:
                    logger.error(f"Signal Evaluation Error: {e}")

            await asyncio.sleep(0.001)

if __name__ == "__main__":
    bridge = RealtimeScalpBridge()
    asyncio.run(bridge.run())