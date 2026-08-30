// مسیر: dashboard/src/app/statement/page.tsx
'use client';

import { useEffect, useState } from 'react';
import Header from '../../components/Header';
import { 
  Download, 
  TrendingUp, 
  ShieldAlert, 
  Award, 
  Percent, 
  DollarSign, 
  Receipt,
  RefreshCw,
  Activity
} from 'lucide-react';

interface MetricsData {
  total_trades: number;
  win_rate: number;
  profit_factor: number;
  sharpe_ratio: number;
  sortino_ratio: number;
  max_drawdown_pct: number;
  net_pnl: number;
  total_fees: number;
  expectancy: number;
  trades: any[];
}

export default function StatementPage() {
  const [data, setData] = useState<MetricsData | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchMetrics = async () => {
    try {
      setLoading(true);
      const res = await fetch('http://localhost:8000/api/statement/metrics');
      if (res.ok) {
        const json = await res.json();
        setData(json);
      }
    } catch (e) {
      console.error('Failed to fetch statement metrics', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchMetrics();
    const interval = setInterval(fetchMetrics, 5000); // به‌روزرسانی زنده هر ۵ ثانیه
    return () => clearInterval(interval);
  }, []);

  const handleDownloadCsv = () => {
    window.open('http://localhost:8000/api/statement/export-csv', '_blank');
  };

  return (
    <main className="min-h-screen bg-slate-950 flex flex-col font-sans">
      <Header />

      <div className="p-6 max-w-7xl mx-auto w-full flex-1 space-y-6">
        {/* هدر صفحه استیتمنت و دکمه دانلود */}
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 bg-slate-900/60 border border-slate-800 p-6 rounded-2xl shadow-xl">
          <div>
            <h1 className="text-xl font-bold text-slate-100 flex items-center gap-2.5">
              <Award className="w-6 h-6 text-cyan-400" />
              Audited Quant Performance Statement
            </h1>
            <p className="text-xs text-slate-400 mt-1 font-mono">
              Institutional Trade Ledger & Quantitative Execution Audit (MFE / MAE Analysis)
            </p>
          </div>

          <div className="flex items-center gap-3">
            <button
              onClick={fetchMetrics}
              className="p-2.5 rounded-xl bg-slate-800 text-slate-300 hover:text-cyan-400 border border-slate-700 transition"
              title="Refresh Statement"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            </button>

            <button
              onClick={handleDownloadCsv}
              className="flex items-center gap-2 bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-slate-950 font-bold px-4 py-2.5 rounded-xl text-xs font-mono shadow-lg shadow-cyan-500/20 transition"
            >
              <Download className="w-4 h-4" />
              Export Statement (CSV)
            </button>
          </div>
        </div>

        {/* کارت‌های شاخص‌های مالی وال‌استریت */}
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <DollarSign className="w-3.5 h-3.5 text-cyan-400" />
              Net Realized PnL
            </span>
            <span className={`text-lg font-mono font-extrabold ${(data?.net_pnl || 0) >= 0 ? 'text-emerald-400' : 'text-rose-400'}`}>
              {(data?.net_pnl || 0) >= 0 ? `+$${(data?.net_pnl || 0).toFixed(2)}` : `-$${Math.abs(data?.net_pnl || 0).toFixed(2)}`}
            </span>
          </div>

          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <Percent className="w-3.5 h-3.5 text-emerald-400" />
              Win Rate %
            </span>
            <span className="text-lg font-mono font-extrabold text-slate-100">
              {(data?.win_rate || 0).toFixed(1)}%
            </span>
          </div>

          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <TrendingUp className="w-3.5 h-3.5 text-blue-400" />
              Profit Factor
            </span>
            <span className="text-lg font-mono font-extrabold text-cyan-400">
              {(data?.profit_factor || 0).toFixed(2)}
            </span>
          </div>

          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <Award className="w-3.5 h-3.5 text-amber-400" />
              Sharpe Ratio
            </span>
            <span className="text-lg font-mono font-extrabold text-amber-400">
              {(data?.sharpe_ratio || 0).toFixed(2)}
            </span>
          </div>

          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <ShieldAlert className="w-3.5 h-3.5 text-rose-400" />
              Max Drawdown
            </span>
            <span className="text-lg font-mono font-extrabold text-rose-400">
              {(data?.max_drawdown_pct || 0).toFixed(2)}%
            </span>
          </div>

          <div className="bg-slate-900/40 border border-slate-800 p-4 rounded-xl shadow-lg">
            <span className="text-[11px] font-mono text-slate-400 flex items-center gap-1.5 mb-2">
              <Receipt className="w-3.5 h-3.5 text-slate-400" />
              Total Fees Paid
            </span>
            <span className="text-lg font-mono font-extrabold text-slate-400">
              ${(data?.total_fees || 0).toFixed(2)}
            </span>
          </div>
        </div>

        {/* جدول سوابق تفصیلی دفتر کل معاملات */}
        <div className="bg-slate-900/40 border border-slate-800 rounded-2xl p-6 shadow-xl">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-semibold text-slate-300 uppercase tracking-wider font-mono">
              Immutable Trade Ledger ({data?.total_trades || 0} Executed Scalps)
            </h2>
            <span className="text-xs text-slate-500 font-mono">
              Expectancy: <strong className="text-emerald-400">${(data?.expectancy || 0).toFixed(4)}</strong> / trade
            </span>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs font-mono">
              <thead>
                <tr className="border-b border-slate-800 text-slate-500">
                  <th className="pb-3">TRADE ID</th>
                  <th className="pb-3">PAIR</th>
                  <th className="pb-3">SIDE</th>
                  <th className="pb-3">ENTRY</th>
                  <th className="pb-3">EXIT</th>
                  <th className="pb-3">NOTIONAL</th>
                  <th className="pb-3">FEE</th>
                  <th className="pb-3">NET PNL</th>
                  <th className="pb-3 text-cyan-400">MFE / MAE</th>
                  <th className="pb-3">EXIT REASON</th>
                  <th className="pb-3 text-right">TIME (UTC)</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800/60">
                {!data?.trades || data.trades.length === 0 ? (
                  <tr>
                    <td colSpan={11} className="py-12 text-center text-slate-500 italic">
                      No trades recorded in ledger yet. Run system to populate audited history...
                    </td>
                  </tr>
                ) : (
                  data.trades.map((t: any, idx: number) => {
                    const isProfit = Number(t.net_pnl ?? 0) >= 0;
                    const side = (t.action || t.side || 'BUY').toUpperCase();
                    const tradeId = t.trade_id || t.id || `tr_${idx}`;
                    const fee = Number(t.total_fee ?? t.fee_paid ?? 0);
                    const mfe = Number(t.mfe_bps ?? 0);
                    const mae = Number(t.mae_bps ?? 0);

                    return (
                      <tr key={tradeId} className="hover:bg-slate-900/60 transition">
                        <td className="py-3 text-slate-500 font-mono">{String(tradeId).slice(0, 8)}...</td>
                        <td className="py-3 font-bold text-slate-200">{t.symbol}</td>
                        <td className="py-3">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-bold ${
                            side.includes('BUY') ? 'bg-emerald-950 text-emerald-400 border border-emerald-800' : 'bg-rose-950 text-rose-400 border border-rose-800'
                          }`}>
                            {side}
                          </span>
                        </td>
                        <td className="py-3 text-slate-300">${Number(t.entry_price || 0).toFixed(2)}</td>
                        <td className="py-3 text-slate-300">${Number(t.exit_price || 0).toFixed(2)}</td>
                        <td className="py-3 text-slate-400">${Number(t.notional_usd || t.margin_allocated || 0).toLocaleString()}</td>
                        <td className="py-3 text-slate-500">${fee.toFixed(2)}</td>
                        <td className={`py-3 font-bold ${isProfit ? 'text-emerald-400' : 'text-rose-400'}`}>
                          {isProfit ? `+$${Number(t.net_pnl || 0).toFixed(2)}` : `-$${Math.abs(Number(t.net_pnl || 0)).toFixed(2)}`}
                        </td>
                        <td className="py-3 text-slate-400 font-mono">
                          <span className="text-emerald-400">+{mfe.toFixed(1)}</span> / <span className="text-rose-400">-{mae.toFixed(1)}</span> bps
                        </td>
                        <td className="py-3">
                          <span className="text-[10px] px-2 py-0.5 rounded bg-slate-800 text-slate-300 border border-slate-700">
                            {t.exit_reason || 'NORMAL_CLOSE'}
                          </span>
                        </td>
                        <td className="py-3 text-right text-slate-500 font-mono">
                          {t.closed_at || t.exit_timestamp ? new Date(t.closed_at || t.exit_timestamp).toLocaleTimeString() : 'N/A'}
                        </td>
                      </tr>
                    );
                  })
                )}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </main>
  );
}