// مسیر: dashboard/src/components/Header.tsx
'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { Activity, CandlestickChart, LayoutDashboard, Zap } from 'lucide-react';

export default function Header() {
  const pathname = usePathname();

  return (
    <header className="border-b border-slate-800 bg-slate-900/50 backdrop-blur px-6 py-3.5 flex items-center justify-between">
      <div className="flex items-center space-x-6">
        <div className="flex items-center space-x-3">
          <div className="bg-cyan-500/10 p-2 rounded-lg border border-cyan-500/20">
            <Zap className="w-5 h-5 text-cyan-400" />
          </div>
          <div>
            <h1 className="font-bold text-base tracking-wide text-slate-100 flex items-center gap-2">
              MI-EDTE <span className="text-cyan-400 text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-950 border border-cyan-800">QUANT TERMINAL</span>
            </h1>
          </div>
        </div>

        {/* منوی جابجایی بین داشبورد و چارت */}
        <nav className="flex items-center space-x-1 bg-slate-950 p-1 rounded-lg border border-slate-800 text-xs font-mono">
          <Link
            href="/"
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md transition ${
              pathname === '/' ? 'bg-slate-800 text-cyan-400 font-bold' : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <LayoutDashboard className="w-3.5 h-3.5" />
            Dashboard
          </Link>
          <Link
            href="/chart"
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md transition ${
              pathname === '/chart' ? 'bg-slate-800 text-cyan-400 font-bold' : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <CandlestickChart className="w-3.5 h-3.5" />
            Live Trading Chart
          </Link>
        </nav>
      </div>

      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-2 bg-emerald-500/10 border border-emerald-500/20 px-3 py-1 rounded-full">
          <Activity className="w-3.5 h-3.5 text-emerald-400 animate-pulse" />
          <span className="text-xs font-mono text-emerald-400">100% Real-Time Live</span>
        </div>
      </div>
    </header>
  );
}