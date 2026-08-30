// مسیر: dashboard/src/components/SignalsView.tsx
'use client';

import React from 'react';
import { Zap } from 'lucide-react';
import { ScalpSignal } from '../context/TerminalContext';

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
          Live Net-EV Signals (Order Flow Imbalance Engine)
        </h2>
        <div className="flex items-center space-x-6 text-xs font-mono">
          <div>
            <span className="text-slate-500 mr-2">Balance:</span>
            <span className="text-slate-200 font-bold">${(cashBalance || 100).toLocaleString('en-US', { minimumFractionDigits: 2 })}</span>
          </div>
          <div>
            <span className="text-slate-500 mr-2">Realized PnL:</span>
            <span className={(totalPnL || 0) >= 0 ? 'text-emerald-400 font-bold' : 'text-rose-400 font-bold'}>
              {(totalPnL || 0) >= 0 ? `+$${(totalPnL || 0).toFixed(2)}` : `-$${Math.abs(totalPnL || 0).toFixed(2)}`}
            </span>
          </div>
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-left text-xs font-mono">
          <thead>
            <tr className="border-b border-slate-800 text-slate-500">
              <th className="pb-3">PAIR</th>
              <th className="pb-3">ACTION</th>
              <th className="pb-3">SIGNAL PRICE</th>
              <th className="pb-3 text-emerald-400">TP TARGET (+6 BPS)</th>
              <th className="pb-3 text-rose-400">SL TARGET (-12 BPS)</th>
              <th className="pb-3">P(TP) PROB%</th>
              <th className="pb-3">NET EXPECTED VALUE</th>
              <th className="pb-3 text-right">TIME (UTC)</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60">
            {!signals || signals.length === 0 ? (
              <tr>
                <td colSpan={8} className="py-8 text-center text-slate-500 italic">
                  Awaiting positive Net-EV opportunities (&gt; +1.2 bps Hurdle)...
                </td>
              </tr>
            ) : (
              signals.map((s, idx) => {
                const isBuy = s?.action === 'BUY';
                const entryPrice = Number(s?.signal_price || 0);
                
                const tpPrice = entryPrice > 0 
                  ? (isBuy ? entryPrice * 1.0006 : entryPrice * 0.9994)
                  : 0;
                const slPrice = entryPrice > 0 
                  ? (isBuy ? entryPrice * 0.9988 : entryPrice * 1.0012)
                  : 0;

                const prob = Number((s as any)?.p_tp ?? s?.probability ?? 0) * 100;
                const ev = Number((s as any)?.expected_net_ev_bps ?? 0);

                return (
                  <tr key={s?.signal_id || idx} className="hover:bg-slate-900/60 transition">
                    <td className="py-3 font-bold text-slate-200">{s?.symbol || 'N/A'}</td>
                    <td className="py-3">
                      <span className={`px-2.5 py-1 rounded text-[10px] font-bold border ${
                        isBuy 
                          ? 'bg-emerald-950 text-emerald-400 border-emerald-800' 
                          : 'bg-rose-950 text-rose-400 border-rose-800'
                      }`}>
                        {s?.action || 'NO_ACTION'}
                      </span>
                    </td>
                    <td className="py-3 text-slate-200 font-bold">
                      {entryPrice > 0 ? `$${entryPrice.toLocaleString('en-US', { minimumFractionDigits: 2 })}` : 'Market'}
                    </td>
                    <td className="py-3 text-emerald-400 font-bold">
                      {tpPrice > 0 ? `$${tpPrice.toLocaleString('en-US', { minimumFractionDigits: 2 })}` : 'N/A'}
                    </td>
                    <td className="py-3 text-rose-400 font-bold">
                      {slPrice > 0 ? `$${slPrice.toLocaleString('en-US', { minimumFractionDigits: 2 })}` : 'N/A'}
                    </td>
                    <td className="py-3 text-cyan-400 font-bold">{prob.toFixed(1)}%</td>
                    <td className="py-3 text-slate-400">
                      EV: <strong className={ev > 0 ? 'text-emerald-400' : 'text-amber-400'}>{ev > 0 ? `+${ev.toFixed(2)}` : ev.toFixed(2)} bps</strong>
                    </td>
                    <td className="py-3 text-right text-slate-500">
                      {s?.timestamp ? new Date(s.timestamp).toLocaleTimeString() : 'N/A'}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}