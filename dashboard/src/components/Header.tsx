// مسیر: dashboard/src/components/Header.tsx
'use client';

import { useState, useEffect } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { 
  Activity, 
  CandlestickChart, 
  LayoutDashboard, 
  FileText, 
  Zap, 
  ShieldAlert, 
  Sliders, 
  Check 
} from 'lucide-react';

export default function Header() {
  const pathname = usePathname();

  // استیت‌های تنظیمات داینامیک سرمایه
  const [totalCapital, setTotalCapital] = useState<number>(100);
  const [allocationPct, setAllocationPct] = useState<number>(0.30);
  const [leverage, setLeverage] = useState<number>(1.0);
  const [killSwitch, setKillSwitch] = useState<boolean>(false);
  const [isSaved, setIsSaved] = useState<boolean>(false);

  useEffect(() => {
    // خواندن تنظیمات فعلی از سرور
    fetch('http://localhost:8000/api/config/capital')
      .then((res) => res.json())
      .then((data) => {
        if (data) {
          setTotalCapital(data.total_capital);
          setAllocationPct(data.allocation_pct);
          setLeverage(data.leverage);
          setKillSwitch(data.kill_switch);
        }
      })
      .catch((e) => console.error('Failed to fetch capital config', e));
  }, []);

  const handleUpdateConfig = async (newKillState?: boolean) => {
    const payload = {
      total_capital: Number(totalCapital),
      allocation_pct: Number(allocationPct),
      leverage: Number(leverage),
      kill_switch: newKillState !== undefined ? newKillState : killSwitch,
    };

    try {
      const res = await fetch('http://localhost:8000/api/config/capital', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (res.ok) {
        setIsSaved(true);
        setTimeout(() => setIsSaved(false), 2000);
      }
    } catch (e) {
      console.error('Error saving config', e);
    }
  };

  return (
    <header className="border-b border-slate-800 bg-slate-900/60 backdrop-blur px-6 py-3 flex flex-wrap items-center justify-between gap-4">
      <div className="flex items-center space-x-6">
        <div className="flex items-center space-x-3">
          <div className="bg-cyan-500/10 p-2 rounded-lg border border-cyan-500/20">
            <Zap className="w-5 h-5 text-cyan-400" />
          </div>
          <div>
            <h1 className="font-bold text-base tracking-wide text-slate-100 flex items-center gap-2">
              MI-EDTE <span className="text-cyan-400 text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-950 border border-cyan-800">DYNAMIC ENGINE</span>
            </h1>
          </div>
        </div>

        {/* منوی جابجایی بین صفحات */}
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
            Live Chart
          </Link>
          <Link
            href="/statement"
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md transition ${
              pathname === '/statement' ? 'bg-slate-800 text-cyan-400 font-bold' : 'text-slate-400 hover:text-slate-200'
            }`}
          >
            <FileText className="w-3.5 h-3.5" />
            Statement & Audit
          </Link>
        </nav>
      </div>

      {/* کنترل پنل مدیریت داینامیک سرمایه و ریسک */}
      <div className="flex items-center gap-3 bg-slate-950/80 border border-slate-800 p-1.5 rounded-xl text-xs font-mono">
        {/* ورود سرمایه */}
        <div className="flex items-center gap-1.5 px-2">
          <span className="text-slate-500">Capital:</span>
          <span className="text-cyan-400 font-bold">$</span>
          <input
            type="number"
            value={totalCapital}
            onChange={(e) => setTotalCapital(Number(e.target.value))}
            className="w-16 bg-slate-900 border border-slate-700 rounded px-1.5 py-0.5 text-slate-100 font-bold text-center focus:outline-none focus:border-cyan-500"
          />
        </div>

        {/* درصد تخصیص در هر ترید */}
        <div className="flex items-center gap-1.5 px-2 border-l border-slate-800">
          <span className="text-slate-500">Alloc:</span>
          <select
            value={allocationPct}
            onChange={(e) => setAllocationPct(Number(e.target.value))}
            className="bg-slate-900 border border-slate-700 rounded px-1.5 py-0.5 text-slate-200 focus:outline-none"
          >
            <option value={0.20}>20%</option>
            <option value={0.30}>30%</option>
            <option value={0.50}>50%</option>
            <option value={1.00}>100%</option>
          </select>
        </div>

        {/* لوریج */}
        <div className="flex items-center gap-1.5 px-2 border-l border-slate-800">
          <span className="text-slate-500">Lev:</span>
          <select
            value={leverage}
            onChange={(e) => setLeverage(Number(e.target.value))}
            className="bg-slate-900 border border-slate-700 rounded px-1.5 py-0.5 text-slate-200 focus:outline-none"
          >
            <option value={1.0}>1x (Spot)</option>
            <option value={2.0}>2x</option>
            <option value={5.0}>5x</option>
          </select>
        </div>

        {/* دکمه اعمال تنظیمات */}
        <button
          onClick={() => handleUpdateConfig()}
          className="flex items-center gap-1 bg-cyan-600 hover:bg-cyan-500 text-slate-950 font-bold px-2.5 py-1 rounded transition"
          title="Apply Capital Config"
        >
          {isSaved ? <Check className="w-3.5 h-3.5" /> : <Sliders className="w-3.5 h-3.5" />}
          Apply
        </button>

        {/* کلید قطع اضطراری Kill-Switch */}
        <button
          onClick={() => {
            const nextKill = !killSwitch;
            setKillSwitch(nextKill);
            handleUpdateConfig(nextKill);
          }}
          className={`flex items-center gap-1 px-2.5 py-1 rounded font-bold border transition ${
            killSwitch
              ? 'bg-rose-600 text-slate-100 border-rose-500 animate-pulse'
              : 'bg-slate-900 text-slate-400 border-slate-800 hover:text-rose-400'
          }`}
          title="Emergency Trading Kill Switch"
        >
          <ShieldAlert className="w-3.5 h-3.5" />
          {killSwitch ? 'HALTED' : 'KILL'}
        </button>
      </div>
    </header>
  );
}