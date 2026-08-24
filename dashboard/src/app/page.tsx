import Header from '../components/Header';
import MarketView from '../components/MarketView';
import NewsFeed from '../components/NewsFeed';
import SignalsView from '../components/SignalsView';

export default function DashboardPage() {
  return (
    <main className="min-h-screen bg-slate-950 flex flex-col">
      <Header />
      <div className="p-6 grid grid-cols-1 md:grid-cols-2 gap-6 max-w-7xl mx-auto w-full flex-1">
        <MarketView />
        <NewsFeed />
        <SignalsView />
      </div>
    </main>
  );
}