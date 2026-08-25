# مسیر: python_engine/app/gateway.py
import os
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

app = FastAPI(title="MI-EDTE Dynamic Gateway & Ledger", version="3.0.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

REDIS_URL = "redis://127.0.0.1:6379"
LEDGER_FILE = "trade_ledger.json"

trade_ledger = deque(maxlen=2000)

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
                data = json.load(f)
                for item in data:
                    trade_ledger.append(item)
            logger.info(f"Loaded {len(trade_ledger)} past trades from ledger.")
        except Exception as e:
            logger.warning(f"Could not load ledger: {e}")

load_initial_ledger()

def persist_trade(trade_record: dict):
    trade_ledger.append(trade_record)
    try:
        with open(LEDGER_FILE, "w", encoding="utf-8") as f:
            json.dump(list(trade_ledger), f, indent=2)
    except Exception as e:
        logger.error(f"Error saving ledger: {e}")

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
    logger.success(f"Dynamic Capital Config Updated & Published: {payload}")
    return {"status": "SUCCESS", "config": current_config}

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
    fees = [float(t.get("fee_paid", 0.0)) for t in trades]

    total_net_pnl = sum(pnls)
    total_fees = sum(fees)
    win_rate = (len(wins) / len(pnls)) * 100.0 if pnls else 0.0
    profit_factor = (sum(wins) / abs(sum(losses))) if losses and sum(losses) != 0 else (sum(wins) if wins else 0.0)

    returns_pct = [float(t.get("pnl_percent", 0.0)) / 100.0 for t in trades]
    std_ret = np.std(returns_pct) if len(returns_pct) > 1 else 0.0001
    mean_ret = np.mean(returns_pct) if returns_pct else 0.0
    sharpe_ratio = float((mean_ret / std_ret) * np.sqrt(252 * 50)) if std_ret > 0 else 0.0

    neg_rets = [r for r in returns_pct if r < 0]
    downside_std = np.std(neg_rets) if len(neg_rets) > 1 else 0.0001
    sortino_ratio = float((mean_ret / downside_std) * np.sqrt(252 * 50)) if downside_std > 0 else 0.0

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
        "sortino_ratio": round(sortino_ratio, 2),
        "max_drawdown_pct": round(max_dd * 100.0, 2),
        "net_pnl": round(total_net_pnl, 2),
        "total_fees": round(total_fees, 2),
        "expectancy": round(expectancy, 2),
        "trades": trades[::-1]
    }

@app.get("/api/statement/export-csv")
async def export_csv():
    trades = list(trade_ledger)
    output = io.StringIO()
    writer = csv.writer(output)
    
    writer.writerow([
        "Trade ID", "Symbol", "Action", "Entry Price", "Exit Price",
        "Quantity", "Notional (USD)", "Fee Paid (USD)", "Net PnL (USD)",
        "PnL %", "Duration (Sec)", "Exit Reason", "Opened At", "Closed At"
    ])
    
    for t in trades:
        writer.writerow([
            t.get("id"), t.get("symbol"), t.get("action"), t.get("entry_price"), t.get("exit_price"),
            t.get("quantity"), t.get("notional_usd"), t.get("fee_paid"), t.get("net_pnl"),
            f"{t.get('pnl_percent')}%", t.get("duration_seconds"), t.get("exit_reason"),
            t.get("opened_at"), t.get("closed_at")
        ])
    
    filename = f"MI_EDTE_Statement_{datetime.now().strftime('%Y%m%d_%H%M%S')}.csv"
    return Response(
        content=output.getvalue(),
        media_type="text/csv",
        headers={"Content-Disposition": f"attachment; filename={filename}"}
    )

async def redis_ledger_listener():
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    await pubsub.subscribe("market:trade_ledger")
    logger.info("Gateway listening to 'market:trade_ledger'...")
    
    while True:
        try:
            msg = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.1)
            if msg:
                data = json.loads(msg["data"])
                persist_trade(data)
            await asyncio.sleep(0.01)
        except Exception:
            await asyncio.sleep(1.0)

@app.on_event("startup")
async def startup_event():
    asyncio.create_task(redis_ledger_listener())

@app.websocket("/ws/live-terminal")
async def live_terminal_websocket(websocket: WebSocket):
    await websocket.accept()
    logger.info("Dashboard connected to Central WebSocket.")
    
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    
    await pubsub.subscribe(
        "market:trades:btcusdt",
        "market:trades:ethusdt",
        "market:scalp_signals",
        "market:positions"
    )

    try:
        past_news = await client.xrevrange("events:news_raw", "+", "-", count=8)
        for _, fields in reversed(past_news):
            payload_str = fields.get("payload", "{}")
            try:
                news_payload = json.loads(payload_str)
            except Exception:
                news_payload = {"raw": payload_str}

            if websocket.client_state == WebSocketState.CONNECTED:
                await websocket.send_json({"channel": "events:news_raw", "data": news_payload})
    except Exception:
        pass

    last_news_id = "$"

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

            streams = await client.xread({"events:news_raw": last_news_id}, count=2, block=10)
            if streams and websocket.client_state == WebSocketState.CONNECTED:
                for _, messages in streams:
                    for msg_id, fields in messages:
                        last_news_id = msg_id
                        try:
                            payload = json.loads(fields.get("payload", "{}"))
                        except Exception:
                            payload = {}
                        await websocket.send_json({"channel": "events:news_raw", "data": payload})

            await asyncio.sleep(0.005)
    except (WebSocketDisconnect, RuntimeError):
        logger.info("Dashboard disconnected cleanly.")
    except Exception as e:
        logger.error(f"WebSocket error: {e}")
    finally:
        await pubsub.close()
        await client.close()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("gateway:app", host="127.0.0.1", port=8000, reload=True)