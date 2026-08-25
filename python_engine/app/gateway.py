# مسیر: python_engine/app/gateway.py
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
import redis.asyncio as aioredis
import json
import asyncio
from loguru import logger
from starlette.websockets import WebSocketState

app = FastAPI(title="MI-EDTE Live Scalping & AI Gateway", version="1.2.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

REDIS_URL = "redis://127.0.0.1:6379"

@app.get("/api/health")
async def health_check():
    return {"status": "ONLINE", "engine": "MI-EDTE Scalp Gateway Active"}

@app.websocket("/ws/live-terminal")
async def live_terminal_websocket(websocket: WebSocket):
    await websocket.accept()
    logger.info("New dashboard client connected to WebSocket.")
    
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    
    await pubsub.subscribe(
        "market:trades:btcusdt",
        "market:trades:ethusdt",
        "market:scalp_signals",
        "market:positions"
    )

    # ارسال ۱۰ خبر اخیر به محض اتصال
    try:
        past_news = await client.xrevrange("events:news_raw", "+", "-", count=10)
        for _, fields in reversed(past_news):
            payload_str = fields.get("payload", "{}")
            try:
                news_payload = json.loads(payload_str)
            except Exception:
                news_payload = {"raw": payload_str}

            if websocket.client_state == WebSocketState.CONNECTED:
                await websocket.send_json({
                    "channel": "events:news_raw",
                    "data": news_payload
                })
    except Exception as e:
        logger.warning(f"Error fetching initial news history: {e}")

    last_news_id = "$"

    try:
        while True:
            if websocket.client_state != WebSocketState.CONNECTED:
                break

            # دریافت پیام‌های زنده
            message = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.02)
            if message and websocket.client_state == WebSocketState.CONNECTED:
                channel = message["channel"]
                raw_text = message["data"]
                try:
                    raw_data = json.loads(raw_text)
                except Exception:
                    raw_data = {"raw": raw_text}

                await websocket.send_json({"channel": channel, "data": raw_data})

            # دریافت استریم اخبار
            streams = await client.xread({"events:news_raw": last_news_id}, count=3, block=10)
            if streams and websocket.client_state == WebSocketState.CONNECTED:
                for _, messages in streams:
                    for msg_id, fields in messages:
                        last_news_id = msg_id
                        payload_str = fields.get("payload", "{}")
                        try:
                            news_payload = json.loads(payload_str)
                        except Exception:
                            news_payload = {"raw": payload_str}

                        await websocket.send_json({
                            "channel": "events:news_raw",
                            "data": news_payload
                        })

            await asyncio.sleep(0.005)
    except (WebSocketDisconnect, RuntimeError):
        logger.info("Dashboard client disconnected cleanly.")
    except Exception as e:
        logger.error(f"WebSocket gateway error: {e}")
    finally:
        await pubsub.close()
        await client.close()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("gateway:app", host="127.0.0.1", port=8000, reload=True)