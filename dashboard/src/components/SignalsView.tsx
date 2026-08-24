'use client';

import { useEffect, useState } from 'react';
import { ShieldCheck, TrendingUp, AlertCircle } from 'lucide-react';

interface SignalItem {
  symbol: string;
  action: string;
  confidence: number;
  reason_codes: string[];
  decision_ts: string;
}

export default function SignalsView() {
  const [signals, setSignals] = useState<SignalItem[]>([]);
  const [paperStats, setPaperStats] = useState({
    cash: 100000.0,
    pnl: 0.0,
    openPositions: 0
  });

  useEffect(() => {
    const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');

    ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        if (parsed.channel === 'market:signals') {
          setSignals((prev) => [parsed.data, ...prev.slice(0, 9)]); // نگهداری ۱۰ سیگنال آخر
        }
      } catch (e) {
        console.error('WS Parse error', e);
      }
    };

    return () => ws.close();
  }, []);

  return (
    <div className="bg-slate-900/40 border border-slate-800 rounded-xl p-5 col-span-full shadow-xl">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-2">
          <ShieldCheck className="w-4 h-4 text-emerald-400" />
          Live Paper Trading & Signal Terminal
        </h2>
        <div className="flex items-center space-x-6 text-xs font-mono">
          <div>
            <span className="text-slate-500 mr-2">Paper Cash:</span>
            <span className="text-slate-200 font-bold">${paperStats.cash.toLocaleString()}</span>
          </div>
          <div>
            <span className="text-slate-500 mr-2">Live PnL:</span>
            <span className={paperStats.pnl >= 0 ? 'text-emerald-400 font-bold' : 'text-rose-400 font-bold'}>
              ${paperStats.pnl.toFixed(2)}
            </span>
          </div>
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-left text-xs">
          <thead>
            <tr className="border-b border-slate-800 text-slate-500 font-mono">
              <th className="pb-3">SYMBOL</th>
              <th className="pb-3">ACTION</th>
              <th className="pb-3">CONFIDENCE</th>
              <th className="pb-3">REASON CODES</th>
              <th className="pb-3 text-right">TIMESTAMP</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800/60 font-mono">
            {signals.length === 0 ? (
              <tr>
                <td colSpan={5} className="py-6 text-center text-slate-500 italic">
                  Awaiting live market events & intelligence triggers... (System evaluating NO_TRADE / HOLD)
                </td>
              </tr>
            ) : (
              signals.map((s, idx) => (
                <tr key={idx} className="hover:bg-slate-900/50 transition">
                  <td className="py-3 font-bold text-slate-200">{s.symbol}</td>
                  <td className="py-3">
                    <span className={`px-2.5 py-1 rounded text-[10px] font-bold ${
                      s.action === 'BUY' ? 'bg-emerald-950 text-emerald-400 border border-emerald-800' :
                      s.action === 'SELL' ? 'bg-rose-950 text-rose-400 border border-rose-800' :
                      'bg-slate-800 text-slate-400 border border-slate-700'
                    }`}>
                      {s.action}
                    </span>
                  </td>
                  <td className="py-3 text-cyan-400">{(s.confidence * 100).toFixed(1)}%</td>
                  <td className="py-3 text-slate-400">{s.reason_codes?.join(', ') || 'N/A'}</td>
                  <td className="py-3 text-right text-slate-500">{new Date(s.decision_ts).toLocaleTimeString()}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}