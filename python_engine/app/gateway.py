# مسیر: python_engine/app/gateway.py
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
import redis.asyncio as aioredis
import json
import asyncio
from loguru import logger

app = FastAPI(title="MI-EDTE Live Scalping & AI Gateway", version="1.0.0")

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
    
    # سابسکرایب به تمامی کانال‌های ترید، متریک، سیگنال اسکلپ و تغییرات پوزیشن
    await pubsub.subscribe(
        "market:trades:btcusdt",
        "market:metrics:btcusdt",
        "market:scalp_signals",
        "market:positions"
    )

    last_news_id = "$"  # فقط اخبار جدید پس از اتصال

    try:
        while True:
            # ۱. خواندن رویدادهای زنده Pub/Sub
            message = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.05)
            if message:
                channel = message["channel"]
                raw_text = message["data"]
                try:
                    raw_data = json.loads(raw_text)
                except Exception:
                    raw_data = {"raw": raw_text}

                await websocket.send_json({"channel": channel, "data": raw_data})

            # ۲. خواندن استریم اخبار پردازش‌شده از Redis Stream
            streams = await client.xread({"events:news_raw": last_news_id}, count=5, block=20)
            if streams:
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
    except WebSocketDisconnect:
        logger.warning("Dashboard client disconnected.")
    except Exception as e:
        logger.error(f"WebSocket gateway error: {e}")
    finally:
        await pubsub.close()
        await client.close()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("gateway:app", host="127.0.0.1", port=8000, reload=True)