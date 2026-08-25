// مسیر: dashboard/src/app/chart/page.tsx
'use client';

import Header from '../../components/Header';
import LiveTradeChart from '../../components/LiveTradeChart';

export default function ChartPage() {
  return (
    <main className="min-h-screen bg-slate-950 flex flex-col h-screen overflow-hidden">
      <Header />
      <div className="flex-1 w-full h-full">
        <LiveTradeChart />
      </div>
    </main>
  );
}