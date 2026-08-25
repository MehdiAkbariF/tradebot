// مسیر: dashboard/src/app/page.tsx
'use client';

import Header from '../components/Header';
import MarketView from '../components/MarketView';
import NewsFeed from '../components/NewsFeed';
import SignalsView from '../components/SignalsView';
import { useTerminal } from '../context/TerminalContext';

export default function DashboardPage() {
  const terminal = useTerminal();

  return (
    <main className="min-h-screen bg-slate-950 flex flex-col font-sans">
      <Header />
      <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-6 max-w-7xl mx-auto w-full flex-1">
        <MarketView 
          ticks={terminal.ticks} 
          isConnected={terminal.isConnected}
          cashBalance={terminal.cashBalance}
        />
        <NewsFeed newsList={terminal.news} />
        <SignalsView 
          signals={terminal.signals} 
          cashBalance={terminal.cashBalance} 
          totalPnL={terminal.totalPnL} 
        />
      </div>
    </main>
  );
}