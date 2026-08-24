import { Activity, ShieldAlert, Zap } from 'lucide-react';

export default function Header() {
  return (
    <header className="border-b border-slate-800 bg-slate-900/50 backdrop-blur px-6 py-4 flex items-center justify-between">
      <div className="flex items-center space-x-3">
        <div className="bg-cyan-500/10 p-2 rounded-lg border border-cyan-500/20">
          <Zap className="w-6 h-6 text-cyan-400" />
        </div>
        <div>
          <h1 className="font-bold text-lg tracking-wide text-slate-100">MI-EDTE <span className="text-cyan-400 text-xs font-mono ml-2 px-2 py-0.5 rounded bg-cyan-950 border border-cyan-800">MVP v0.4</span></h1>
          <p className="text-xs text-slate-400">Market Intelligence & Event-Driven Terminal</p>
        </div>
      </div>

      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-2 bg-emerald-500/10 border border-emerald-500/20 px-3 py-1.5 rounded-full">
          <Activity className="w-4 h-4 text-emerald-400 animate-pulse" />
          <span className="text-xs font-medium text-emerald-400">Engine Live</span>
        </div>

        <div className="flex items-center space-x-2 bg-slate-800/80 border border-slate-700 px-3 py-1.5 rounded-full">
          <ShieldAlert className="w-4 h-4 text-amber-400" />
          <span className="text-xs font-medium text-slate-300">Risk: OK</span>
        </div>
      </div>
    </header>
  );
}