export default function NewsFeed() {
  const mockNews = [
    { id: 1, title: "Bitcoin breaks above 200-day moving average for first time in weeks", asset: "BTCUSDT", rel: "0.85", time: "2m ago" },
    { id: 2, title: "Optimism moves 546.9M OP from future airdrops to ecosystem growth fund", asset: "OPUSDT", rel: "0.85", time: "8m ago" },
    { id: 3, title: "US debt tops $40T stoking debate on what it means for safe-haven assets", asset: "GENERAL", rel: "0.30", time: "15m ago" },
  ];

  return (
    <div className="bg-slate-900/40 border border-slate-800 rounded-xl p-5 flex flex-col">
      <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider mb-4">Live News Intelligence</h2>
      <div className="space-y-3 overflow-y-auto max-h-[300px] pr-1">
        {mockNews.map((n) => (
          <div key={n.id} className="p-3 bg-slate-900/80 border border-slate-800/80 rounded-lg hover:border-slate-700 transition">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-950 text-cyan-400 border border-cyan-900">{n.asset}</span>
              <span className="text-xs text-slate-500">{n.time}</span>
            </div>
            <p className="text-xs text-slate-300 font-medium leading-relaxed">{n.title}</p>
          </div>
        ))}
      </div>
    </div>
  );
}