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

class LocalCandleBuilder:
    def __init__(self, max_candles=60):
        self.candles = deque(maxlen=max_candles)
        self.current_candle = None
        self.current_minute = None

    def add_tick(self, price: float, vol: float, ts: datetime):
        minute = ts.strftime("%Y-%m-%d %H:%M")
        if self.current_minute != minute:
            if self.current_candle:
                self.candles.append(self.current_candle)
            self.current_minute = minute
            self.current_candle = {
                "open": price, "high": price, "low": price, "close": price, "vol": vol
            }
        else:
            self.current_candle["high"] = max(self.current_candle["high"], price)
            self.current_candle["low"] = min(self.current_candle["low"], price)
            self.current_candle["close"] = price
            self.current_candle["vol"] += vol

    def get_df(self) -> pd.DataFrame:
        data = list(self.candles)
        if self.current_candle:
            data.append(self.current_candle)
        if not data:
            return pd.DataFrame()
        return pd.DataFrame(data)

class RealtimeScalpBridge:
    def __init__(self):
        self.price_history = {}
        self.volume_history = {}
        self.buy_vol_history = {}
        self.sell_vol_history = {}
        self.candle_builders = {
            "BTCUSDT": LocalCandleBuilder(max_candles=60),
            "ETHUSDT": LocalCandleBuilder(max_candles=60)
        }
        self.active_news = deque(maxlen=50)
        self.decay_lambda = 0.0069
        self.last_signal_time = {}
        self.last_heartbeat_time = 0.0

    def get_decayed_sentiment(self) -> float:
        now = datetime.now(timezone.utc)
        total = 0.0
        for n in self.active_news:
            delta = (now - n["ts"]).total_seconds()
            if delta <= 600:
                decay = np.exp(-self.decay_lambda * delta)
                total += n["score"] * decay
        return float(np.clip(total, -1.0, 1.0))

    def evaluate_live_market(self, symbol: str, price: float, vol: float, is_buyer_maker: bool) -> dict | None:
        now = datetime.now(timezone.utc)
        
        builder = self.candle_builders.get(symbol)
        if builder:
            builder.add_tick(price, vol, now)

        if symbol not in self.price_history:
            self.price_history[symbol] = deque(maxlen=1000)
            self.volume_history[symbol] = deque(maxlen=1000)
            self.buy_vol_history[symbol] = deque(maxlen=1000)
            self.sell_vol_history[symbol] = deque(maxlen=1000)
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

        if len(hist_p) < 30:
            return None

        prices = list(hist_p)
        p_now = prices[-1]
        p_5s = prices[-6] if len(prices) >= 6 else prices[0]
        ret_5s = (p_now - p_5s) / p_5s if p_5s > 0 else 0.0

        # ۱. محاسبه دامنه نوسان واقعی ۳۰ تیک اخیر
        sub_30 = prices[-30:]
        volatility_bps = float(((max(sub_30) - min(sub_30)) / sub_30[0]) * 10000.0) if len(sub_30) > 1 else 0.0

        # فیلتر بازار بدون نوسان (زیر 0.60 پیپ)
        if volatility_bps < 0.60:
            now_sec = now.timestamp()
            if (now_sec - self.last_heartbeat_time) > 8.0:
                self.last_heartbeat_time = now_sec
                logger.info(f"💤 STANDBY (Low Volatility): {symbol} Vol={volatility_bps:.2f} bps | Waiting for range expansion...")
            return None

        # ۲. فیلتر ترند قدرتمند بر اساس میانگین متحرک ۳۰۰ تیک اخیر (جلوگیری از خرید در ریزش)
        long_window = min(len(prices), 300)
        ma_long = float(np.mean(prices[-long_window:]))
        
        if p_now > ma_long:
            trend_bias = 1.0
            trend_str = "BULLISH 🟢"
        else:
            trend_bias = -1.0
            trend_str = "BEARISH 🔴"

        # ۳. محاسبه عدم تعادل بوک (OFI) با ۶۰ تیک اخیر جهت حذف نویز تک‌تیک‌ها
        ofi_window = min(len(hist_bv), 60)
        b_vol = sum(list(hist_bv)[-ofi_window:])
        s_vol = sum(list(hist_sv)[-ofi_window:])
        tot_vol = b_vol + s_vol
        live_ofi = float((b_vol - s_vol) / tot_vol) if tot_vol > 0 else 0.0

        decayed_sentiment = self.get_decayed_sentiment()
        now_sec = now.timestamp()

        # محاسبه امتیاز آلفا
        alpha_score = (trend_bias * 0.45) + (live_ofi * 0.40) + (np.clip(ret_5s * 2500.0, -0.20, 0.20)) + (decayed_sentiment * 0.05)

        if (now_sec - self.last_heartbeat_time) > 5.0:
            self.last_heartbeat_time = now_sec
            logger.info(
                f"📊 LIVE SCAN: {symbol} = ${p_now:,.2f} | "
                f"Trend: {trend_str} | "
                f"Range: {volatility_bps:.2f} bps | "
                f"OFI: {live_ofi:+.2f} | "
                f"Alpha: {alpha_score:+.2f}"
            )

        # ۴. صدور سیگنال اسکلپ همراه با کول‌داون ۳۰ ثانیه‌ای
        if (now_sec - self.last_signal_time[symbol]) > 30.0:
            # شرط ورود BUY: فقط در صورت ترند صعودی، قیمت بالای میانگین و فشار خرید قوی
            if trend_bias > 0 and p_now >= ma_long and alpha_score >= 0.35 and live_ofi > 0.25:
                self.last_signal_time[symbol] = now_sec
                prob = round(0.75 + (alpha_score * 0.20), 4)
                return {
                    "symbol": symbol,
                    "action": "BUY",
                    "probability": prob,
                    "trend_bias": 1.0,
                    "decayed_sentiment": round(decayed_sentiment, 2),
                    "timestamp": now.isoformat()
                }
            # شرط ورود SELL: فقط در صورت ترند نزولی، قیمت زیر میانگین و فشار فروش قوی
            elif trend_bias < 0 and p_now <= ma_long and alpha_score <= -0.35 and live_ofi < -0.25:
                self.last_signal_time[symbol] = now_sec
                prob = round(0.75 + (abs(alpha_score) * 0.20), 4)
                return {
                    "symbol": symbol,
                    "action": "SELL",
                    "probability": prob,
                    "trend_bias": -1.0,
                    "decayed_sentiment": round(decayed_sentiment, 2),
                    "timestamp": now.isoformat()
                }

        return None

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe("market:trades:btcusdt", "market:trades:ethusdt")

        logger.info("Realtime Scalp Alpha Engine ACTIVE (Macro Trend & 60-Tick OFI)...")

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
                        logger.info(f"🎯 MAKER SCALP SIGNAL: {signal['symbol']} -> {signal['action']} ({signal['probability']*100:.1f}%)")
                except Exception as e:
                    logger.error(f"Error processing market message: {e}")

            # فید اخبار
            news_streams = await client.xread({"events:news_raw": "$"}, count=2, block=10)
            if news_streams:
                for _, messages in news_streams:
                    for _, fields in messages:
                        payload = json.loads(fields.get("payload", "{}"))
                        title = payload.get("title", "").lower()
                        
                        bullish_words = ["surge", "jump", "record", "etf", "approval", "rally", "gain", "inflow", "sec approves"]
                        bearish_words = ["crash", "drop", "hack", "lawsuit", "ban", "sec sues", "outflow", "plunge"]
                        
                        pos_count = sum(1 for w in bullish_words if w in title)
                        neg_count = sum(1 for w in bearish_words if w in title)
                        
                        score = 0.0
                        if pos_count > neg_count: score = min(0.3 * pos_count, 0.9)
                        elif neg_count > pos_count: score = max(-0.3 * neg_count, -0.9)
                        
                        self.active_news.append({
                            "ts": datetime.now(timezone.utc),
                            "score": score,
                            "title": payload.get("title", "")
                        })

            await asyncio.sleep(0.001)

if __name__ == "__main__":
    bridge = RealtimeScalpBridge()
    asyncio.run(bridge.run())