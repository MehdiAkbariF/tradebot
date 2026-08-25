// مسیر: dashboard/src/hooks/useLiveTerminal.ts
import { useEffect, useState, useRef } from 'react';

export interface ScalpSignal {
  symbol: string;
  action: string;
  probability: number;
  trend_bias: number;
  decayed_sentiment: number;
  timestamp: string;
}

export interface TradeTick {
  symbol: string;
  price: string;
  quantity: string;
  exchange_ts: string;
}

export interface NewsItem {
  id: string;
  title: string;
  url?: string;
  source_id?: string;
  time: string;
}

export interface PositionUpdate {
  symbol: string;
  action: string;
  entry_price: number;
  exit_price?: number;
  pnl?: number;
  reason?: string;
}

export function useLiveTerminal() {
  const [isConnected, setIsConnected] = useState(false);
  const [ticks, setTicks] = useState<Record<string, TradeTick>>({});
  const [signals, setSignals] = useState<ScalpSignal[]>([]);
  const [news, setNews] = useState<NewsItem[]>([]);
  const [positions, setPositions] = useState<PositionUpdate[]>([]);
  const [totalPnL, setTotalPnL] = useState<number>(0);
  const [cashBalance, setCashBalance] = useState<number>(100000);

  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');
    wsRef.current = ws;

    ws.onopen = () => setIsConnected(true);
    ws.onclose = () => setIsConnected(false);

    ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        const { channel, data } = parsed;

        if (channel.startsWith('market:trades:')) {
          const sym = data.symbol.toUpperCase();
          setTicks((prev) => ({ ...prev, [sym]: data }));
        } else if (channel === 'market:scalp_signals') {
          setSignals((prev) => [data, ...prev.slice(0, 14)]);
        } else if (channel === 'events:news_raw') {
          const payload = typeof data === 'string' ? JSON.parse(data) : data;
          const newItem: NewsItem = {
            id: payload.id || Math.random().toString(),
            title: payload.title || 'Market Update',
            url: payload.url,
            source_id: payload.source_id || 'Crypto Feed',
            time: new Date().toLocaleTimeString(),
          };
          setNews((prev) => [newItem, ...prev.filter(n => n.title !== newItem.title).slice(0, 9)]);
        } else if (channel === 'market:positions') {
          setPositions((prev) => [data, ...prev.slice(0, 9)]);
          if (data.pnl) {
            setTotalPnL((prev) => prev + Number(data.pnl));
            setCashBalance((prev) => prev + Number(data.pnl));
          }
        }
      } catch (err) {
        console.error('Error parsing WS packet', err);
      }
    };

    return () => {
      ws.close();
    };
  }, []);

  return {
    isConnected,
    ticks,
    signals,
    news,
    positions,
    totalPnL,
    cashBalance,
  };
}