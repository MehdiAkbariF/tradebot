# مسیر: python_engine/app/gateway.py
import sys
import os

BASE_DIR = os.path.dirname(os.path.abspath(__file__))
PYTHON_ENGINE_DIR = os.path.abspath(os.path.join(BASE_DIR, ".."))
if PYTHON_ENGINE_DIR not in sys.path:
    sys.path.insert(0, PYTHON_ENGINE_DIR)
if BASE_DIR not in sys.path:
    sys.path.insert(0, BASE_DIR)

import io
import csv
import json
import asyncio
import numpy as np
from datetime import datetime
from collections import deque
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Response
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel
import redis.asyncio as aioredis
from loguru import logger
from starlette.websockets import WebSocketState

try:
    from app.diagnostics.engine import StrategyDiagnosticEngine
except ImportError:
    try:
        from diagnostics.engine import StrategyDiagnosticEngine
    except ImportError:
        class StrategyDiagnosticEngine:
            def __init__(self, ledger_path): self.ledger_path = ledger_path
            def generate_full_report(self): return {"status": "INITIALIZING", "message": "Collecting trades..."}

app = FastAPI(title="MI-EDTE Production Gateway & Diagnostics", version="4.3.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

REDIS_URL = "redis://127.0.0.1:6379"
LEDGER_FILE = os.path.abspath(os.path.join(BASE_DIR, "..", "trade_ledger.json"))

trade_ledger = deque(maxlen=5000)
recent_signals = deque(maxlen=20)
recent_news = deque(maxlen=15) # 👈 حافظه زنده نگهداری اخبار
diagnostic_engine = StrategyDiagnosticEngine(ledger_path=LEDGER_FILE)

class CapitalConfigSchema(BaseModel):
    total_capital: float = 100.0
    allocation_pct: float = 0.30
    leverage: float = 1.0
    kill_switch: bool = False

current_config = CapitalConfigSchema()

def load_initial_ledger():
    if os.path.exists(LEDGER_FILE):
        try:
            with open(LEDGER_FILE, "r", encoding="utf-8") as f:
                content = f.read().strip()
                if content:
                    data = json.loads(content)
                    for item in data:
                        trade_ledger.append(item)
            logger.info(f"Loaded {len(trade_ledger)} trades into memory.")
        except Exception as e:
            logger.warning(f"Could not load ledger: {e}")

load_initial_ledger()

def persist_trade_atomic(trade_record: dict):
    trade_ledger.append(trade_record)
    temp_file = f"{LEDGER_FILE}.tmp"
    try:
        with open(temp_file, "w", encoding="utf-8") as f:
            json.dump(list(trade_ledger), f, indent=2)
        os.replace(temp_file, LEDGER_FILE)
    except Exception as e:
        logger.error(f"Error saving ledger: {e}")

# ==================== ENDPOINTS ====================

@app.get("/api/health")
async def health_check():
    return {"status": "ONLINE", "total_trades": len(trade_ledger)}

@app.get("/api/config/capital")
async def get_capital_config():
    return current_config

@app.post("/api/config/capital")
async def update_capital_config(cfg: CapitalConfigSchema):
    global current_config
    current_config = cfg
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    payload = {
        "total_capital": str(cfg.total_capital),
        "allocation_pct": cfg.allocation_pct,
        "leverage": cfg.leverage,
        "kill_switch": cfg.kill_switch,
    }
    await client.publish("config:capital_risk", json.dumps(payload))
    await client.close()
    return {"status": "SUCCESS", "config": current_config}

@app.get("/api/diagnostics/report")
async def get_diagnostic_report():
    return diagnostic_engine.generate_full_report()

@app.get("/api/statement/metrics")
async def get_statement_metrics():
    trades = list(trade_ledger)
    if not trades:
        return {
            "total_trades": 0, "win_rate": 0.0, "profit_factor": 0.0,
            "sharpe_ratio": 0.0, "sortino_ratio": 0.0, "max_drawdown_pct": 0.0,
            "net_pnl": 0.0, "total_fees": 0.0, "expectancy": 0.0,
            "trades": []
        }

    pnls = [float(t.get("net_pnl", 0.0)) for t in trades]
    wins = [p for p in pnls if p > 0]
    losses = [p for p in pnls if p < 0]
    fees = [float(t.get("total_fee", t.get("fee_paid", 0.0))) for t in trades]

    total_net_pnl = sum(pnls)
    total_fees = sum(fees)
    win_rate = (len(wins) / len(pnls)) * 100.0 if pnls else 0.0
    profit_factor = (sum(wins) / abs(sum(losses))) if losses and sum(losses) != 0 else (sum(wins) if wins else 0.0)

    returns_pct = [float(t.get("pnl_percent", 0.0)) / 100.0 for t in trades]
    std_ret = np.std(returns_pct) if len(returns_pct) > 1 else 0.0001
    mean_ret = np.mean(returns_pct) if returns_pct else 0.0
    sharpe_ratio = float((mean_ret / std_ret) * np.sqrt(252 * 50)) if std_ret > 0 else 0.0

    curr = current_config.total_capital
    peak = current_config.total_capital
    max_dd = 0.0
    for p in pnls:
        curr += p
        peak = max(peak, curr)
        dd = (peak - curr) / peak if peak > 0 else 0.0
        max_dd = max(max_dd, dd)

    avg_win = np.mean(wins) if wins else 0.0
    avg_loss = abs(np.mean(losses)) if losses else 0.0
    expectancy = ((win_rate/100.0) * avg_win) - (((100.0 - win_rate)/100.0) * avg_loss)

    return {
        "total_trades": len(trades),
        "win_rate": round(win_rate, 2),
        "profit_factor": round(profit_factor, 2),
        "sharpe_ratio": round(sharpe_ratio, 2),
        "sortino_ratio": 0.0,
        "max_drawdown_pct": round(max_dd * 100.0, 2),
        "net_pnl": round(total_net_pnl, 2),
        "total_fees": round(total_fees, 2),
        "expectancy": round(expectancy, 4),
        "trades": trades[::-1]
    }

@app.get("/api/statement/export-csv")
async def export_csv():
    trades = list(trade_ledger)
    output = io.StringIO()
    writer = csv.writer(output)
    
    writer.writerow([
        "Trade ID", "Signal ID", "Symbol", "Action", "Strategy Version",
        "Signal Price", "Order Price", "Fill Price", "Exit Price",
        "Quantity", "Notional (USD)", "Entry Fee", "Exit Fee", "Total Fee",
        "Slippage (BPS)", "Gross PnL", "Net PnL", "PnL %", "PnL (BPS)",
        "MFE (BPS)", "MAE (BPS)", "Duration (Sec)", "Exit Reason",
        "Alpha", "OFI", "Confidence", "Signal Time", "Exit Time"
    ])
    
    for t in trades:
        writer.writerow([
            t.get("trade_id", t.get("id")), t.get("signal_id"), t.get("symbol"), t.get("action"), t.get("strategy_version"),
            t.get("signal_price"), t.get("order_price"), t.get("fill_price", t.get("entry_price")), t.get("exit_price"),
            t.get("quantity"), t.get("notional_usd"), t.get("entry_fee"), t.get("exit_fee"), t.get("total_fee", t.get("fee_paid")),
            t.get("slippage_bps"), t.get("gross_pnl"), t.get("net_pnl"), f"{t.get('pnl_percent')}%", t.get("pnl_bps"),
            t.get("mfe_bps"), t.get("mae_bps"), t.get("duration_seconds"), t.get("exit_reason"),
            t.get("alpha_at_signal"), t.get("ofi_at_signal"), t.get("model_confidence"),
            t.get("signal_timestamp", t.get("opened_at")), t.get("exit_timestamp", t.get("closed_at"))
        ])
    
    filename = f"MI_EDTE_Audited_Ledger_{datetime.now().strftime('%Y%m%d_%H%M%S')}.csv"
    return Response(
        content=output.getvalue(),
        media_type="text/csv",
        headers={"Content-Disposition": f"attachment; filename={filename}"}
    )

async def redis_ledger_listener():
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    await pubsub.subscribe("market:trade_ledger", "market:scalp_signals", "events:news_raw")
    logger.info("Gateway listening to ledger, signals, and live news streams...")
    
    while True:
        try:
            msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.1)
            if msg:
                channel = msg["channel"]
                data = json.loads(msg["data"])
                if channel == "market:trade_ledger":
                    persist_trade_atomic(data)
                elif channel == "market:scalp_signals":
                    recent_signals.appendleft(data)
                elif channel == "events:news_raw":
                    payload = data if isinstance(data, dict) else json.loads(data)
                    recent_news.appendleft(payload) # ذخیره خبر برای تحویل به داشبورد
            await asyncio.sleep(0.01)
        except Exception as e:
            await asyncio.sleep(1.0)

@app.on_event("startup")
async def startup_event():
    asyncio.create_task(redis_ledger_listener())

@app.websocket("/ws/live-terminal")
async def live_terminal_websocket(websocket: WebSocket):
    await websocket.accept()
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    
    await pubsub.subscribe(
        "market:trades:btcusdt",
        "market:trades:ethusdt",
        "market:scalp_signals",
        "market:positions",
        "events:news_raw"
    )

    # ⚡ ۱. ارسال فوری تمام سیگنال‌های اخیر به محض باز شدن یا رفرش صفحه
    for sig in list(recent_signals):
        try:
            await websocket.send_json({"channel": "market:scalp_signals", "data": sig})
        except Exception:
            pass

    # ⚡ ۲. ارسال فوری تمام اخبار اخیر به محض باز شدن صفحه
    for n in list(recent_news):
        try:
            await websocket.send_json({"channel": "events:news_raw", "data": n})
        except Exception:
            pass

    try:
        while True:
            if websocket.client_state != WebSocketState.CONNECTED:
                break
            message = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.02)
            if message and websocket.client_state == WebSocketState.CONNECTED:
                try:
                    raw_data = json.loads(message["data"])
                except Exception:
                    raw_data = {"raw": message["data"]}
                await websocket.send_json({"channel": message["channel"], "data": raw_data})
            await asyncio.sleep(0.005)
    except (WebSocketDisconnect, RuntimeError):
        pass
    finally:
        await pubsub.close()
        await client.close()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("gateway:app", host="127.0.0.1", port=8000, reload=True)