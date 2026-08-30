# مسیر: python_engine/app/intelligence/realtime_bridge.py
import json
import os
import uuid
import asyncio
from datetime import datetime, timezone
from collections import deque
import numpy as np
import redis.asyncio as aioredis
from loguru import logger

REDIS_URL = "redis://127.0.0.1:6379"
STRATEGY_VERSION = "canonical_v9.0_pure"
CALIBRATION_FILE = os.path.join(os.path.dirname(__file__), "..", "research", "calibration_matrix.json")
if not os.path.exists(CALIBRATION_FILE):
    CALIBRATION_FILE = "calibration_matrix.json"

MIN_EXECUTABLE_EV_HURDLE = 1.2

class DeterministicGapAwareResampler:
    def __init__(self):
        self.bars_1s = deque(maxlen=900)
        self.bars_1m = deque(maxlen=60)
        
        self.active_sec = None
        self.active_open = None
        self.active_high = -float("inf")
        self.active_low = float("inf")
        self.active_close = None
        self.active_vol = 0.0
        self.active_ofi = 0.0
        
        self.session_day = None
        self.vwap_trade_num = 0.0
        self.vwap_trade_den = 0.0
        
        self.active_min_bucket = None
        self.ema_5m = None
        self.ema_15m = None

    def ingest_market_tick(self, ts_sec: float, price: float, vol: float, l2_ofi_step: float):
        sec_int = int(ts_sec)
        min_int = (sec_int // 60) * 60
        dt = datetime.fromtimestamp(ts_sec, tz=timezone.utc)
        current_day = dt.date()

        if self.session_day != current_day:
            self.session_day = current_day
            self.vwap_trade_num = 0.0
            self.vwap_trade_den = 0.0

        self.vwap_trade_num += (price * vol)
        self.vwap_trade_den += vol

        # هندلینگ گپ‌های ثانیه‌ای
        if self.active_sec is not None and sec_int > self.active_sec + 1:
            last_c = self.active_close if self.active_close is not None else price
            for m_sec in range(self.active_sec + 1, sec_int):
                self.bars_1s.append({
                    "sec": m_sec, "open": last_c, "high": last_c,
                    "low": last_c, "close": last_c, "vol": 0.0, "ofi": 0.0
                })

        if self.active_sec is None or self.active_sec != sec_int:
            if self.active_sec is not None:
                self.bars_1s.append({
                    "sec": self.active_sec, "open": self.active_open, "high": self.active_high,
                    "low": self.active_low, "close": self.active_close,
                    "vol": self.active_vol, "ofi": self.active_ofi
                })
            self.active_sec = sec_int
            self.active_open = price
            self.active_high = price
            self.active_low = price
            self.active_close = price
            self.active_vol = vol
            self.active_ofi = l2_ofi_step
        else:
            self.active_high = max(self.active_high, price)
            self.active_low = min(self.active_low, price)
            self.active_close = price
            self.active_vol += vol
            self.active_ofi += l2_ofi_step

        if self.active_min_bucket is None:
            self.active_min_bucket = min_int
        elif min_int > self.active_min_bucket:
            step_min = 60
            for gap_min in range(self.active_min_bucket, min_int, step_min):
                self.finalize_1m_bar(self.active_close)
            self.active_min_bucket = min_int

    def finalize_1m_bar(self, close_p: float):
        self.bars_1m.append(close_p)
        alpha_5 = 2.0 / (5.0 + 1.0)
        alpha_15 = 2.0 / (15.0 + 1.0)

        if self.ema_5m is None:
            self.ema_5m = close_p
        else:
            self.ema_5m = (alpha_5 * close_p) + ((1.0 - alpha_5) * self.ema_5m)

        if self.ema_15m is None:
            self.ema_15m = close_p
        else:
            self.ema_15m = (alpha_15 * close_p) + ((1.0 - alpha_15) * self.ema_15m)

    def get_session_vwap(self) -> float:
        return (self.vwap_trade_num / self.vwap_trade_den) if self.vwap_trade_den > 0 else (self.bars_1s[-1]["close"] if self.bars_1s else 0.0)

    def get_macro_trend(self) -> tuple[int, str]:
        if not self.bars_1s or len(self.bars_1s) < 10:
            return 0, f"WARMING_UP ({len(self.bars_1s)}/10s)"

        cur_p = self.bars_1s[-1]["close"]
        vwap = self.get_session_vwap()

        # راه‌اندازی سریع با بارهای ثانیه‌ای تا زمان پر شدن بارهای دقیقه‌ای
        if self.ema_5m is None or self.ema_15m is None:
            recent_closes = [b["close"] for b in list(self.bars_1s)[-60:]]
            fast = float(np.mean(recent_closes[-10:]))
            slow = float(np.mean(recent_closes))
            if cur_p >= vwap and fast >= slow:
                return 1, "BULLISH (Fast Bootstrap)"
            elif cur_p <= vwap and fast <= slow:
                return -1, "BEARISH (Fast Bootstrap)"
            else:
                return 0, "CHOP (Fast Bootstrap)"

        if cur_p > vwap and self.ema_5m > self.ema_15m:
            return 1, "BULLISH"
        elif cur_p < vwap and self.ema_5m < self.ema_15m:
            return -1, "BEARISH"
        else:
            return 0, "CHOP"

    def compute_ker_and_vol(self) -> tuple[float, float]:
        if len(self.bars_1s) < 10: return 1.0, 0.50
        recent = list(self.bars_1s)[-60:]
        closes = [b["close"] for b in recent]
        
        log_rets = [np.log(closes[i] / closes[i-1]) for i in range(1, len(closes))]
        vol_bps = float(np.std(log_rets) * 10000.0) if log_rets else 1.0

        net_change = abs(closes[-1] - closes[0])
        total_path = sum(abs(closes[i] - closes[i-1]) for i in range(1, len(closes)))
        ker = (net_change / total_path) if total_path > 0 else 0.50
        return max(vol_bps, 1.0), float(ker)


class CanonicalQuantEngineV9:
    def __init__(self):
        self.resamplers = {}
        self.last_rust_l2_metrics = {}
        self.last_signal_ts = {}
        self.last_heartbeat_ts = {}
        self.calibration_db = self.load_or_create_calibration_db()

    def load_or_create_calibration_db(self) -> dict:
        if os.path.exists(CALIBRATION_FILE):
            try:
                with open(CALIBRATION_FILE, "r", encoding="utf-8") as f:
                    logger.info(f"Loaded Calibration DB from {CALIBRATION_FILE}")
                    return json.load(f)
            except Exception as e:
                logger.error(f"Error loading calibration matrix: {e}")
        
        # در صورت نبود فایل، دیتابیس کالیبراسیون تجربی پیش‌فرض را بساز
        logger.warning("Calibration DB not found. Generating initial empirical matrix...")
        from app.research.build_calibration_db import build_4d_empirical_calibration_db
        build_4d_empirical_calibration_db(CALIBRATION_FILE)
        with open(CALIBRATION_FILE, "r", encoding="utf-8") as f:
            return json.load(f)

    def get_calibrated_probabilities(self, regime: str, ofi_z: float, ker: float, vol_bps: float) -> tuple[float, float, float, float] | None:
        abs_z = abs(ofi_z)
        z_bin = "2.5+" if abs_z >= 2.5 else ("2.0-2.5" if abs_z >= 2.0 else ("1.5-2.0" if abs_z >= 1.5 else "1.0-1.5"))
        ker_bin = "0.6+" if ker >= 0.6 else ("0.4-0.6" if ker >= 0.4 else "0.2-0.4")
        vol_bin = "HIGH_VOL" if vol_bps >= 2.0 else "LOW_VOL"
        reg_clean = "BULLISH" if "BULLISH" in regime else ("BEARISH" if "BEARISH" in regime else "CHOP")

        key = f"{reg_clean}|{z_bin}|{ker_bin}|{vol_bin}"
        entry = self.calibration_db.get(key)
        if not entry:
            return 0.45, 0.25, 0.30, -0.50

        return entry["p_tp"], entry["p_sl"], entry["p_timeout"], entry["expected_timeout_return_bps"]

    def compute_conditional_friction(self, spread_bps: float, vol_bps: float, p_tp: float) -> float:
        maker_fee = 0.20
        taker_fee = 0.50
        expected_exit_fee = (p_tp * maker_fee) + ((1.0 - p_tp) * taker_fee)
        slippage_penalty = 0.10 * (vol_bps / 2.0)
        half_spread = spread_bps / 2.0
        adverse_selection = 0.30
        return float(maker_fee + expected_exit_fee + half_spread + slippage_penalty + adverse_selection)

    def evaluate_tick(self, symbol: str, price: float, vol: float, ts_sec: float) -> dict | None:
        now_ts = datetime.now(timezone.utc).timestamp()

        if symbol not in self.resamplers:
            self.resamplers[symbol] = DeterministicGapAwareResampler()
            self.last_rust_l2_metrics[symbol] = {}
            self.last_signal_ts[symbol] = 0.0
            self.last_heartbeat_ts[symbol] = 0.0

        metrics = self.last_rust_l2_metrics.get(symbol, {})
        l2_ofi_step = float(metrics.get("ofi", 0.0))
        rust_ofi_zscore = float(metrics.get("ofi_zscore", 0.0))
        spread_bps = float(metrics.get("spread_bps", 0.50))

        resampler = self.resamplers[symbol]
        resampler.ingest_market_tick(ts_sec, price, vol, l2_ofi_step)

        macro_bias, macro_desc = resampler.get_macro_trend()
        vol_bps, ker = resampler.compute_ker_and_vol()

        # 💓 لاگ ضربان قلب در بالای متد قرار گرفت تا همیشه وضعیت را ببینید
        if (now_ts - self.last_heartbeat_ts[symbol]) >= 3.0:
            self.last_heartbeat_ts[symbol] = now_ts
            bars_count = len(resampler.bars_1m)
            logger.info(f"🧭 [{symbol}] Trend: {macro_desc} (1M Bars: {bars_count}) | True L2 OFI Z: {rust_ofi_zscore:+.2f} | KER: {ker:.2f}")

        if macro_bias == 0: return None
        if ker < 0.15: return None

        action = None
        if macro_bias == 1 and rust_ofi_zscore >= 1.0:
            action = "BUY"
        elif macro_bias == -1 and rust_ofi_zscore <= -1.0:
            action = "SELL"
        else:
            return None

        calib = self.get_calibrated_probabilities(macro_desc, rust_ofi_zscore, ker, vol_bps)
        if not calib: return None
        p_tp, p_sl, p_to, e_timeout_bps = calib

        tp_bps = max(7.0, round(vol_bps * 1.8, 1))
        sl_bps = max(4.5, round(vol_bps * 1.1, 1))

        gross_ev_bps = (p_tp * tp_bps) - (p_sl * sl_bps) + (p_to * e_timeout_bps)
        friction_bps = self.compute_conditional_friction(spread_bps, vol_bps, p_tp)
        executable_net_ev_bps = gross_ev_bps - friction_bps

        if (now_ts - self.last_signal_ts[symbol]) >= 5.0:
            if executable_net_ev_bps >= MIN_EXECUTABLE_EV_HURDLE:
                self.last_signal_ts[symbol] = now_ts

                return {
                    "signal_id": str(uuid.uuid4()),
                    "strategy_version": STRATEGY_VERSION,
                    "symbol": symbol,
                    "action": action,
                    "expected_net_ev_bps": round(float(executable_net_ev_bps), 2),
                    "p_tp": round(float(p_tp), 4),
                    "p_sl": round(float(p_sl), 4),
                    "p_timeout": round(float(p_to), 4),
                    "tp_bps": tp_bps,
                    "sl_bps": sl_bps,
                    "friction_bps": round(float(friction_bps), 2),
                    "signal_price": str(price),
                    "timestamp": datetime.fromtimestamp(ts_sec, tz=timezone.utc).isoformat()
                }

        return None

    async def run(self):
        client = aioredis.from_url(REDIS_URL, decode_responses=True)
        pubsub = client.pubsub()
        await pubsub.subscribe(
            "market:trades:btcusdt", "market:trades:ethusdt",
            "market:metrics:btcusdt", "market:metrics:ethusdt"
        )
        logger.info("⚡ Canonical v9.0 Engine ACTIVE (Instant Bootstrap + Live Logging)...")

        while True:
            try:
                msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.01)
                if msg and msg.get("data"):
                    channel = msg["channel"]
                    data = json.loads(msg["data"])

                    if channel.startswith("market:metrics:"):
                        sym = data.get("symbol", "").upper()
                        self.last_rust_l2_metrics[sym] = data

                    elif channel.startswith("market:trades:"):
                        sym = data.get("symbol", "").upper()
                        price = float(data.get("price", 0.0))
                        vol = float(data.get("quantity", 0.0))
                        
                        exchange_ts = data.get("exchange_ts")
                        if isinstance(exchange_ts, str):
                            try: ts_sec = datetime.fromisoformat(exchange_ts.replace("Z", "+00:00")).timestamp()
                            except Exception: ts_sec = datetime.now(timezone.utc).timestamp()
                        else:
                            ts_sec = float(exchange_ts) if exchange_ts else datetime.now(timezone.utc).timestamp()

                        signal = self.evaluate_tick(sym, price, vol, ts_sec)
                        if signal:
                            await client.publish("market:scalp_signals", json.dumps(signal))
                            logger.success(f"🎯 EXECUTABLE SIGNAL: {signal['symbol']} {signal['action']} [TP: +{signal['tp_bps']}bps | SL: -{signal['sl_bps']}bps | EV: +{signal['expected_net_ev_bps']}bps]")
            except Exception as e:
                logger.error(f"Error in signal loop: {e}")
            await asyncio.sleep(0.001)

if __name__ == "__main__":
    engine = CanonicalQuantEngineV9()
    asyncio.run(engine.run())