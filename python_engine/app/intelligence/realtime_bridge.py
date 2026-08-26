# مسیر: python_engine/app/intelligence/realtime_bridge.py
import json
import os
import asyncio
import numpy as np
import pandas as pd
import redis.asyncio as aioredis
from datetime import datetime, timezone
from collections import deque
from loguru import logger

REDIS_URL = "redis://127.0.0.1:6379"

class RealtimeScalpBridge:
    def __init__(self):
        self.price_history = {}
        self.buy_vol_history = {}
        self.sell_vol_history = {}
        self.last_signal_time = {}
        self.last_heartbeat_time = 0.0

    def evaluate_live_market(self, symbol: str, price: float, vol: float, is_buyer_maker: bool) -> dict | None:
        now = datetime.now(timezone.utc)
        
        if symbol not in self.price_history:
            self.price_history[symbol] = deque(maxlen=200)
            self.buy_vol_history[symbol] = deque(maxlen=200)
            self.sell_vol_history[symbol] = deque(maxlen=200)
            self.last_signal_time[symbol] = 0.0

        hist_p = self.price_history[symbol]
        hist_bv = self.buy_vol_history[symbol]
        hist_sv = self.sell_vol_history[symbol]
        
        hist_p.append(price)
        if not is_buyer_maker:
            hist_bv.append(vol)
            hist_sv.append(0.0)
        else:
            hist_bv.append(0.0)
            hist_sv.append(vol)

        if len(hist_p) < 40:
            return None

        prices = list(hist_p)
        
        # 🧠 مدل Market Making: بررسی خستگی در 40 تیک اخیر
        p_now = prices[-1]
        
        # OFI فوق‌سریع (20 تیک)
        b_vol = sum(list(hist_bv)[-20:])
        s_vol = sum(list(hist_sv)[-20:])
        tot_vol = b_vol + s_vol
        live_ofi = float((b_vol - s_vol) / tot_vol) if tot_vol > 0 else 0.0

        # محاسبه Micro-Price Drift (انحراف قیمت نسبت به میانگین خرد)
        micro_mean = np.mean(prices[-20:])
        price_drift = (p_now - micro_mean) / micro_mean * 10000.0 # به BPS

        now_sec = now.timestamp()
        
        # فرمول آربیتراژ آماری:
        # اگر خریداران در اوردربوک زیادند (OFI مثبت) اما قیمت نتوانسته رشد کند (Drift منفی)، یعنی دیوار فروش وجود دارد -> SELL!
        # اگر فروشندگان حمله‌ور شده‌اند (OFI منفی) اما قیمت نمی‌ریزد -> BUY!
        
        alpha_score = (live_ofi * 0.60) - (price_drift * 0.40)

        if (now_sec - self.last_heartbeat_time) > 2.0:
            self.last_heartbeat_time = now_sec
            logger.info(f"🔬 MICRO-SCAN: {symbol} | OFI: {live_ofi:+.2f} | Drift: {price_drift:+.2f} bps | Alpha: {alpha_score:+.2f}")

        # 🚀 شلیک سیگنال رگباری هر 3 ثانیه!
        if (now_sec - self.last_signal_time[symbol]) > 3.0:
            # 💡 سیگنال معکوس مارکت‌میکینگ (Mean Reversion)
            if live_ofi < -0.40 and price_drift > -0.10 and alpha_score <= -0.50:
                self.last_signal_time[symbol] = now_sec
                return {
                    "symbol": symbol,
                    "action": "BUY",
                    "probability": 0.95,
                    "trend_bias": 1.0,
                    "decayed_sentiment": 0.0,
                    "timestamp": now.isoformat()
                }
            elif live_ofi > 0.40 and price_drift < 0.10 and alpha_score >= 0.50:
                self.last_signal_time[symbol] = now_sec
                return {
                    "symbol": symbol,
                    "action": "SELL",
                    "probability": 0.95,
                    "trend_bias": -1.0,
                    "decayed_sentiment": 0.0,
                    "timestamp": now.isoformat()
                }

        return None

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe("market:trades:btcusdt", "market:trades:ethusdt")

        logger.info("🔥 STATISTICAL ARBITRAGE (MARKET MAKER) ENGINE ACTIVE...")

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
                        logger.success(f"⚡ MM LIQUIDITY SIGNAL: Provide {signal['action']} on {signal['symbol']}")
                except Exception as e:
                    logger.error(f"Error processing market message: {e}")

            await asyncio.sleep(0.001)

if __name__ == "__main__":
    bridge = RealtimeScalpBridge()
    asyncio.run(bridge.run())