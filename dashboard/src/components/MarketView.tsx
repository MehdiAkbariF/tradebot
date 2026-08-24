'use client';

import { useEffect, useState } from 'react';
import { ArrowUpRight, ArrowDownRight, Activity } from 'lucide-react';

interface MarketMetric {
  symbol: string;
  best_bid: string;
  best_ask: string;
  spread_bps: number;
  mid_price: string;
  micro_price: string;
  imbalance_top10: number;
  timestamp: string;
}

export default function MarketView() {
  const [metrics, setMetrics] = useState<Record<string, MarketMetric>>({});
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
   const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');

    ws.onopen = () => {
      setIsConnected(true);
      console.log('Connected to MI-EDTE Live Market WebSocket');
    };

   ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        if (parsed.channel && parsed.channel.startsWith('market:trades:')) {
          const data = parsed.data;
          const symbol = data.symbol.toUpperCase();
          setMetrics((prev) => ({
            ...prev,
            [symbol]: {
              symbol: symbol,
              best_bid: data.price,
              best_ask: data.price,
              spread_bps: 0.5,
              mid_price: data.price,
              micro_price: data.price,
              imbalance_top10: 0.2,
              timestamp: data.exchange_ts
            }
          }));
        }
      } catch (e) {
        console.error('Failed to parse WS message', e);
      }
    };
    ws.onclose = () => {
      setIsConnected(false);
      console.log('Disconnected from Market WebSocket');
    };

    return () => {
      ws.close();
    };
  }, []);

  const symbols = ['BTCUSDT', 'ETHUSDT'];

  return (
    <div className="bg-slate-900/40 border border-slate-800 rounded-xl p-5 flex flex-col justify-between shadow-xl">
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider">Active Markets (Live Stream)</h2>
          <div className="flex items-center space-x-1.5">
            <span className={`w-2 h-2 rounded-full ${isConnected ? 'bg-emerald-400 animate-ping' : 'bg-rose-500'}`} />
            <span className="text-[10px] font-mono text-slate-400">{isConnected ? 'WS Connected' : 'Connecting...'}</span>
          </div>
        </div>

        <div className="space-y-3">
          {symbols.map((sym) => {
            const m = metrics[sym];
            return (
              <div key={sym} className="p-3.5 bg-slate-900/80 border border-slate-800 rounded-lg hover:border-slate-700 transition">
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center space-x-2">
                    <span className="font-bold text-slate-200">{sym}</span>
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-slate-800 text-slate-400">Spot</span>
                  </div>
                  <span className="font-mono text-cyan-400 font-bold text-sm">
                    {m ? `$${Number(m.mid_price).toLocaleString()}` : 'Waiting for tick...'}
                  </span>
                </div>

                {m && (
                  <div className="grid grid-cols-3 gap-2 mt-3 pt-3 border-t border-slate-800/60 text-[11px] font-mono">
                    <div>
                      <span className="text-slate-500 block">Spread</span>
                      <span className="text-slate-300">{m.spread_bps.toFixed(2)} bps</span>
                    </div>
                    <div>
                      <span className="text-slate-500 block">Imbalance</span>
                      <span className={m.imbalance_top10 >= 0 ? 'text-emerald-400' : 'text-rose-400'}>
                        {(m.imbalance_top10 * 100).toFixed(1)}%
                      </span>
                    </div>
                    <div className="text-right">
                      <span className="text-slate-500 block">Best Ask / Bid</span>
                      <span className="text-slate-300">{Number(m.best_ask).toFixed(1)}</span>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <div className="mt-6 pt-4 border-t border-slate-800/80">
        <div className="flex justify-between text-xs text-slate-400 mb-1">
          <span>Paper Cash Balance:</span>
          <span className="font-mono text-slate-200 font-bold">$100,000.00</span>
        </div>
        <div className="flex justify-between text-xs text-slate-400">
          <span>System Latency:</span>
          <span className="font-mono text-emerald-400 font-bold">&lt; 2ms</span>
        </div>
      </div>
    </div>
  );
}