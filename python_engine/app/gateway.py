from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.middleware.cors import CORSMiddleware
import redis.asyncio as aioredis
import json
import asyncio

app = FastAPI(title="MI-EDTE Live Paper Trading Gateway", version="0.4.0")

app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

REDIS_URL = "redis://127.0.0.1:6379"

@app.get("/api/health")
async def health_check():
    return {"status": "HEALTHY", "engine": "MI-EDTE Paper Trading Active"}

@app.websocket("/ws/live-terminal")
async def live_terminal_websocket(websocket: WebSocket):
    await websocket.accept()
    client = aioredis.from_url(REDIS_URL, decode_responses=True)
    pubsub = client.pubsub()
    
    # سابسکرایب به کانال‌های ترید، سیگنال و اخبار
    await pubsub.subscribe(
        "market:trades:btcusdt",
        "market:trades:ethusdt",
        "market:signals",
        "events:news_raw"
    )

    try:
        while True:
            message = await pubsub.get_message(ignore_subscribe_messages=True, timeout=0.5)
            if message:
                channel = message["channel"]
                raw_data = json.loads(message["data"])
                
                # تفکیک کانال‌ها برای ارسال ساختاریافته به داشبورد
                payload = {
                    "channel": channel,
                    "data": raw_data
                }
                await websocket.send_json(payload)
            await asyncio.sleep(0.01)
    except WebSocketDisconnect:
        print("Terminal client disconnected.")
    finally:
        await pubsub.close()
        await client.close()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run("gateway:app", host="127.0.0.1", port=8000, reload=True)