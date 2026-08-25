'use client';

import { useEffect, useState } from 'react';
import { Newspaper, Rss } from 'lucide-react';

interface NewsItem {
  id: string;
  title: string;
  url?: string;
  source_id?: string;
  time: string;
}

export default function NewsFeed() {
  const [newsList, setNewsList] = useState<NewsItem[]>([]);
  const [isConnected, setIsConnected] = useState(false);

  useEffect(() => {
    const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');

    ws.onopen = () => setIsConnected(true);
    ws.onclose = () => setIsConnected(false);

    ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        
        // اگر پیام مربوط به فید اخبار خام باشد
        if (parsed.channel === 'events:news_raw') {
          const d = parsed.data;
          const payload = typeof d === 'string' ? JSON.parse(d) : (d.payload ? JSON.parse(d.payload) : d);
          
          const newItem: NewsItem = {
            id: payload.id || Math.random().toString(),
            title: payload.title || 'Untitled News',
            url: payload.url,
            source_id: payload.source_id || 'RSS Feed',
            time: new Date().toLocaleTimeString()
          };

          setNewsList((prev) => [newItem, ...prev.slice(0, 9)]); // نگهداری ۱۰ خبر آخر
        }
      } catch (e) {
        console.error('News WS parse error', e);
      }
    };

    return () => ws.close();
  }, []);

  return (
    <div className="bg-slate-900/40 border border-slate-800 rounded-xl p-5 flex flex-col shadow-xl">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider flex items-center gap-2">
          <Newspaper className="w-4 h-4 text-cyan-400" />
          Live News Intelligence
        </h2>
        <div className="flex items-center space-x-1.5">
          <Rss className={`w-3.5 h-3.5 ${isConnected ? 'text-emerald-400 animate-pulse' : 'text-slate-600'}`} />
          <span className="text-[10px] font-mono text-slate-400">{isConnected ? 'Feed Active' : 'Connecting...'}</span>
        </div>
      </div>

      <div className="space-y-3 overflow-y-auto max-h-[300px] pr-1">
        {newsList.length === 0 ? (
          <div className="py-12 text-center text-slate-500 italic text-xs">
            Listening for live RSS news feeds... (Polling active)
          </div>
        ) : (
          newsList.map((n) => (
            <div key={n.id} className="p-3 bg-slate-900/80 border border-slate-800/80 rounded-lg hover:border-slate-700 transition">
              <div className="flex items-center justify-between mb-1.5">
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-950 text-cyan-400 border border-cyan-900">
                  {n.source_id.includes('coindesk') ? 'CoinDesk' : 'CryptoNews'}
                </span>
                <span className="text-xs text-slate-500 font-mono">{n.time}</span>
              </div>
              <a 
                href={n.url || '#'} 
                target="_blank" 
                rel="noreferrer" 
                className="text-xs text-slate-200 font-medium leading-relaxed hover:text-cyan-400 transition block line-clamp-2"
              >
                {n.title}
              </a>
            </div>
          ))
        )}
      </div>
    </div>
  );
}