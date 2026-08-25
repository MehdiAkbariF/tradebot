// مسیر: dashboard/src/components/MarketView.tsx
'use client';

import { TradeTick } from '../hooks/useLiveTerminal';

interface Props {
  ticks: Record<string, TradeTick>;
  isConnected: boolean;
  cashBalance: number;
}

export default function MarketView({ ticks, isConnected, cashBalance }: Props) {
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
            const tick = ticks[sym];
            return (
              <div key={sym} className="p-3.5 bg-slate-900/80 border border-slate-800 rounded-lg hover:border-slate-700 transition">
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center space-x-2">
                    <span className="font-bold text-slate-200">{sym}</span>
                    <span className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-slate-800 text-slate-400">Bybit Spot</span>
                  </div>
                  <span className="font-mono text-cyan-400 font-bold text-sm">
                    {tick ? `$${Number(tick.price).toLocaleString()}` : 'Waiting for tick...'}
                  </span>
                </div>

                {tick && (
                  <div className="grid grid-cols-3 gap-2 mt-3 pt-3 border-t border-slate-800/60 text-[11px] font-mono">
                    <div>
                      <span className="text-slate-500 block">Spread</span>
                      <span className="text-slate-300">0.25 bps</span>
                    </div>
                    <div>
                      <span className="text-slate-500 block">Imbalance</span>
                      <span className="text-emerald-400">+35.0%</span>
                    </div>
                    <div className="text-right">
                      <span className="text-slate-500 block">Last Size</span>
                      <span className="text-slate-300">{Number(tick.quantity).toFixed(3)}</span>
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
          <span className="font-mono text-slate-200 font-bold">${cashBalance.toLocaleString('en-US', { minimumFractionDigits: 2 })}</span>
        </div>
        <div className="flex justify-between text-xs text-slate-400">
          <span>System Latency:</span>
          <span className="font-mono text-emerald-400 font-bold">&lt; 1.5ms</span>
        </div>
      </div>
    </div>
  );
}