// مسیر: dashboard/src/context/TerminalContext.tsx
'use client';

import React, { createContext, useContext, useEffect, useState, useRef } from 'react';

export interface TradeTick {
  symbol: string;
  price: string;
  quantity: string;
  exchange_ts: string;
}

export interface ScalpSignal {
  signal_id?: string;
  symbol: string;
  action: string;
  probability: number;
  alpha_score?: number;
  ofi?: number;
  range_bps?: number;
  price_drift_bps?: number;
  is_volume_expanding?: boolean;
  signal_price?: string;
  trend_bias?: number;
  decayed_sentiment?: number;
  timestamp: string;
}

export interface NewsItem {
  id: string;
  title: string;
  url?: string;
  source_id?: string;
  time?: string;
}

export interface PositionUpdate {
  symbol: string;
  action: string;
  entry_price: number;
  exit_price?: number;
  pnl?: number;
  reason?: string;
}

interface TerminalContextType {
  isConnected: boolean;
  ticks: Record<string, TradeTick>;
  signals: ScalpSignal[];
  news: NewsItem[];
  positions: PositionUpdate[];
  lastPositionEvent: PositionUpdate | null;
  totalPnL: number;
  cashBalance: number;
  syncCapital: (newCap: number) => void;
}

const TerminalContext = createContext<TerminalContextType | undefined>(undefined);

export function TerminalProvider({ children }: { children: React.ReactNode }) {
  const [isConnected, setIsConnected] = useState(false);
  const [ticks, setTicks] = useState<Record<string, TradeTick>>({});
  const [signals, setSignals] = useState<ScalpSignal[]>([]);
  const [news, setNews] = useState<NewsItem[]>([]);
  const [positions, setPositions] = useState<PositionUpdate[]>([]);
  const [lastPositionEvent, setLastPositionEvent] = useState<PositionUpdate | null>(null);
  const [totalPnL, setTotalPnL] = useState<number>(0);
  const [initialCapital, setInitialCapital] = useState<number>(100);

  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    fetch('http://localhost:8000/api/config/capital')
      .then((res) => res.json())
      .then((data) => {
        if (data && data.total_capital) {
          setInitialCapital(Number(data.total_capital));
        }
      })
      .catch((err) => console.error('Could not fetch capital config', err));
  }, []);

  const syncCapital = (newCap: number) => {
    setInitialCapital(newCap);
  };

  useEffect(() => {
    let reconnectTimeout: any;

    const connect = () => {
      const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');
      wsRef.current = ws;

      ws.onopen = () => {
        setIsConnected(true);
      };

      ws.onclose = () => {
        setIsConnected(false);
        reconnectTimeout = setTimeout(connect, 2000);
      };

      ws.onmessage = (event) => {
        try {
          const parsed = JSON.parse(event.data);
          const { channel, data } = parsed;

          if (channel.startsWith('market:trades:')) {
            const sym = data.symbol.toUpperCase();
            setTicks((prev) => ({ ...prev, [sym]: data }));
          } else if (channel === 'market:scalp_signals') {
            setSignals((prev) => [data, ...prev.filter(s => s.signal_id !== data.signal_id).slice(0, 19)]);
          } else if (channel === 'events:news_raw') {
            const payload = typeof data === 'string' ? JSON.parse(data) : data;
            const newItem: NewsItem = {
              id: payload.id || Math.random().toString(),
              title: payload.title || 'Market Event',
              url: payload.url,
              source_id: payload.source_id || 'Crypto Feed',
              time: new Date().toLocaleTimeString(),
            };
            setNews((prev) => [newItem, ...prev.filter(n => n.title !== newItem.title).slice(0, 14)]);
          } else if (channel === 'market:positions') {
            setLastPositionEvent(data);
            setPositions((prev) => [data, ...prev.slice(0, 9)]);
            if (data.pnl) {
              setTotalPnL((prev) => prev + Number(data.pnl));
            }
          }
        } catch (err) {
          console.error('WS parse error', err);
        }
      };
    };

    connect();

    return () => {
      clearTimeout(reconnectTimeout);
      wsRef.current?.close();
    };
  }, []);

  const currentDynamicBalance = initialCapital + totalPnL;

  return (
    <TerminalContext.Provider
      value={{
        isConnected,
        ticks,
        signals,
        news,
        positions,
        lastPositionEvent,
        totalPnL,
        cashBalance: currentDynamicBalance,
        syncCapital,
      }}
    >
      {children}
    </TerminalContext.Provider>
  );
}

export function useTerminal() {
  const context = useContext(TerminalContext);
  if (!context) {
    throw new Error('useTerminal must be used within a TerminalProvider');
  }
  return context;
}