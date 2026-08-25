// مسیر: dashboard/src/components/SignalsView.tsx
'use client';

import { ShieldCheck, Zap } from 'lucide-react';
import { ScalpSignal } from '../hooks/useLiveTerminal';

interface Props {
  signals: ScalpSignal[];
  cashBalance: number;
  totalPnL: number;
}

export default function SignalsView({ signals, cashBalance, totalPnL }: Props) {
  return (
    <div className="bg-slate-900/40 border border-slate-800 rounded-xl p-5 col-span-full shadow-2xl">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-sm font-semibold text-slate-300 uppercase tracking-wider flex items-center gap-2">
          <Zap className="w-4 h-4 text-cyan-400 fill-cyan-400/20" />
          Live High-Frequency Scalp Signals (LightGBM + Microstructure)
        </h2>
        <div className="flex items-center space-x-6 text-xs font-mono">
          <div>
            <span className="text-slate-500 mr-2">Paper Balance:</span>
            <span className="text-slate-200 font-bold">${cashBalance.toLocaleString('en-US', { minimumFractionDigits: 2 })}</span>
          </div>
          <div>
            <span className="text-slate-500 mr-2">Realized PnL:</span>
            <span className={totalPnL >= 0 ? 'text-emerald-400 font-bold' : 'text-rose-400 font-bold'}>
              {totalPnL >= 0 ? `+$${totalPnL.toFixed(2)}` : `-$${Math.abs(totalPnL).toFixed(2)}`}
            </span>
          </div>
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-left text-xs font-mono">
          <thead>
            <tr className="border-b border-slate-800 text-slate-500">
              <th className="pb-3">SYMBOL</th>
              <th className="pb-3">ACTION</th>
              <th className="pb-3">ML PROBABILITY</th>
              <th className="pb-3">TREND BIAS</th>
              <th className="pb-3">DECAYED SENTIMENT</th>
              <th className="pb-3 text-right">TIMESTAMP</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60">
            {signals.length === 0 ? (
              <tr>
                <td colSpan={6} className="py-8 text-center text-slate-500 italic">
                  Awaiting ML scalp signals (Model scanning 1s/15s microstructure + trend bias)...
                </td>
              </tr>
            ) : (
              signals.map((s, idx) => (
                <tr key={idx} className="hover:bg-slate-900/60 transition">
                  <td className="py-3 font-bold text-slate-200">{s.symbol}</td>
                  <td className="py-3">
                    <span className="px-2.5 py-1 rounded text-[10px] font-bold bg-emerald-950 text-emerald-400 border border-emerald-800">
                      {s.action}
                    </span>
                  </td>
                  <td className="py-3 text-cyan-400 font-bold">{(s.probability * 100).toFixed(1)}%</td>
                  <td className="py-3">
                    <span className={s.trend_bias > 0 ? 'text-emerald-400' : 'text-rose-400'}>
                      {s.trend_bias > 0 ? '15m Bullish' : '15m Bearish'}
                    </span>
                  </td>
                  <td className="py-3 text-slate-300">{s.decayed_sentiment > 0 ? `+${s.decayed_sentiment.toFixed(2)}` : s.decayed_sentiment.toFixed(2)}</td>
                  <td className="py-3 text-right text-slate-500">{new Date(s.timestamp).toLocaleTimeString()}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}