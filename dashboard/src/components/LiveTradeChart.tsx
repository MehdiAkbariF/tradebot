// مسیر: dashboard/src/components/LiveTradeChart.tsx
'use client';

import { useEffect, useRef, useState } from 'react';
import { createChart, IChartApi, ISeriesApi, CandlestickData, SeriesMarker, IPriceLine } from 'lightweight-charts';
import { Activity, ShieldCheck, Zap } from 'lucide-react';

interface PositionData {
  symbol: string;
  action: string;
  entry_price: number;
  tp_price?: number;
  sl_price?: number;
  exit_price?: number;
  pnl?: number;
  reason?: string;
}

export default function LiveTradeChart() {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  
  const [selectedSymbol, setSelectedSymbol] = useState<'BTCUSDT' | 'ETHUSDT'>('BTCUSDT');
  const [currentPrice, setCurrentPrice] = useState<number>(0);
  const [activeTrade, setActiveTrade] = useState<PositionData | null>(null);

  const currentCandleRef = useRef<CandlestickData | null>(null);
  const markersRef = useRef<SeriesMarker<any>[]>([]);
  const tpLineRef = useRef<IPriceLine | null>(null);
  const slLineRef = useRef<IPriceLine | null>(null);

  // ۱. راه‌اندازی اولیه موتور تریدینگ‌ویو
  useEffect(() => {
    if (!chartContainerRef.current) return;

    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { color: '#090d16' },
        textColor: '#94a3b8',
      },
      grid: {
        vertLines: { color: '#1e293b40' },
        horzLines: { color: '#1e293b40' },
      },
      crosshair: {
        mode: 1,
      },
      rightPriceScale: {
        borderColor: '#334155',
        autoScale: true,
      },
      timeScale: {
        borderColor: '#334155',
        timeVisible: true,
        secondsVisible: true,
      },
    });

    const candlestickSeries = chart.addCandlestickSeries({
      upColor: '#10b981',
      downColor: '#f43f5e',
      borderVisible: false,
      wickUpColor: '#10b981',
      wickDownColor: '#f43f5e',
    });

    chartRef.current = chart;
    seriesRef.current = candlestickSeries;

    const handleResize = () => {
      if (chartContainerRef.current) {
        chart.applyOptions({
          width: chartContainerRef.current.clientWidth,
          height: chartContainerRef.current.clientHeight,
        });
      }
    };

    window.addEventListener('resize', handleResize);
    handleResize();

    return () => {
      window.removeEventListener('resize', handleResize);
      chart.remove();
    };
  }, [selectedSymbol]);

  // ۲. اتصال به وب‌سوکت و استریم تیک‌ها و ترسیم معامله
  useEffect(() => {
    const ws = new WebSocket('ws://localhost:8000/ws/live-terminal');

    ws.onmessage = (event) => {
      try {
        const parsed = JSON.parse(event.data);
        const { channel, data } = parsed;

        // الف) پردازش تیک قیمت و ساخت کندل زنده ۵ ثانیه‌ای
        if (channel === `market:trades:${selectedSymbol.toLowerCase()}`) {
          const price = parseFloat(data.price);
          setCurrentPrice(price);

          const nowSec = Math.floor(Date.now() / 1000);
          const candleTime = Math.floor(nowSec / 5) * 5; // کندل‌های ۵ ثانیه‌ای

          if (!currentCandleRef.current || (currentCandleRef.current.time as number) !== candleTime) {
            currentCandleRef.current = {
              time: candleTime as any,
              open: price,
              high: price,
              low: price,
              close: price,
            };
          } else {
            currentCandleRef.current.high = max(currentCandleRef.current.high, price);
            currentCandleRef.current.low = min(currentCandleRef.current.low, price);
            currentCandleRef.current.close = price;
          }

          if (seriesRef.current && currentCandleRef.current) {
            seriesRef.current.update(currentCandleRef.current);
          }
        }

        // ب) رسم نشانگرهای معامله (فلش ورود، حد سود، حد ضرر و خروج)
        if (channel === 'market:positions') {
          const posData: PositionData = data;
          if (posData.symbol.toUpperCase() === selectedSymbol) {
            const nowSec = Math.floor(Date.now() / 1000);

            if (posData.action === 'OPEN') {
              setActiveTrade(posData);

              // رسم فلش ورود روی چارت
              const newMarker: SeriesMarker<any> = {
                time: (Math.floor(nowSec / 5) * 5) as any,
                position: posData.entry_price ? 'belowBar' : 'aboveBar',
                color: '#38bdf8',
                shape: 'arrowUp',
                text: `OPEN ${posData.symbol} @ $${posData.entry_price}`,
              };

              markersRef.current.push(newMarker);
              seriesRef.current?.setMarkers(markersRef.current);

              // رسم خط افقی حد سود (سبز) و حد ضرر (قرمز)
              if (seriesRef.current) {
                if (tpLineRef.current) seriesRef.current.removePriceLine(tpLineRef.current);
                if (slLineRef.current) seriesRef.current.removePriceLine(slLineRef.current);

                const tpPrice = posData.entry_price * 1.0030;
                const slPrice = posData.entry_price * 0.9985;

                tpLineRef.current = seriesRef.current.createPriceLine({
                  price: tpPrice,
                  color: '#10b981',
                  lineWidth: 2,
                  lineStyle: 2,
                  axisLabelVisible: true,
                  title: 'TARGET TP (+0.30%)',
                });

                slLineRef.current = seriesRef.current.createPriceLine({
                  price: slPrice,
                  color: '#f43f5e',
                  lineWidth: 2,
                  lineStyle: 2,
                  axisLabelVisible: true,
                  title: 'STOP LOSS (-0.15%)',
                });
              }
            } else if (posData.action === 'CLOSE') {
              setActiveTrade(null);

              // پاکسازی خطوط TP/SL قبلی
              if (seriesRef.current) {
                if (tpLineRef.current) seriesRef.current.removePriceLine(tpLineRef.current);
                if (slLineRef.current) seriesRef.current.removePriceLine(slLineRef.current);
              }

              // ثبت برچسب خروج و سود نهایی
              const isProfit = (posData.pnl || 0) >= 0;
              const exitMarker: SeriesMarker<any> = {
                time: (Math.floor(nowSec / 5) * 5) as any,
                position: isProfit ? 'aboveBar' : 'belowBar',
                color: isProfit ? '#10b981' : '#f43f5e',
                shape: isProfit ? 'circle' : 'square',
                text: `CLOSED ${posData.reason || ''} | PnL: ${isProfit ? '+' : ''}$${posData.pnl?.toFixed(2)}`,
              };

              markersRef.current.push(exitMarker);
              seriesRef.current?.setMarkers(markersRef.current);
            }
          }
        }
      } catch (err) {
        console.error('Error processing chart websocket data', err);
      }
    };

    return () => {
      ws.close();
    };
  }, [selectedSymbol]);

  function max(a: number, b: number) { return a > b ? a : b; }
  function min(a: number, b: number) { return a < b ? a : b; }

  return (
    <div className="flex flex-col h-full w-full bg-slate-950 p-6 space-y-4">
      {/* هدر کنترل چارت و سوییچ نمادها */}
      <div className="flex items-center justify-between bg-slate-900/60 border border-slate-800 p-4 rounded-xl shadow-xl">
        <div className="flex items-center space-x-4">
          <div className="flex bg-slate-950 p-1 rounded-lg border border-slate-800">
            {(['BTCUSDT', 'ETHUSDT'] as const).map((sym) => (
              <button
                key={sym}
                onClick={() => {
                  setSelectedSymbol(sym);
                  markersRef.current = [];
                  currentCandleRef.current = null;
                }}
                className={`px-4 py-1.5 rounded-md text-xs font-mono font-bold transition ${
                  selectedSymbol === sym
                    ? 'bg-cyan-500 text-slate-950 shadow-lg shadow-cyan-500/20'
                    : 'text-slate-400 hover:text-slate-200'
                }`}
              >
                {sym}
              </button>
            ))}
          </div>

          <div className="flex items-center space-x-2">
            <span className="text-xl font-mono font-extrabold text-cyan-400">
              {currentPrice > 0 ? `$${currentPrice.toLocaleString()}` : 'Connecting stream...'}
            </span>
            <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800 text-emerald-400 border border-emerald-500/30 flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-ping" />
              5s Live Candles
            </span>
          </div>
        </div>

        {/* اطلاعات پوزیشن فعال */}
        <div className="flex items-center space-x-6 text-xs font-mono">
          {activeTrade ? (
            <div className="flex items-center space-x-3 bg-cyan-950/40 border border-cyan-500/30 px-3 py-1.5 rounded-lg">
              <Zap className="w-4 h-4 text-cyan-400 animate-pulse" />
              <span className="text-slate-300">
                Active Position: <strong className="text-cyan-400">{activeTrade.action}</strong> @ ${activeTrade.entry_price}
              </span>
            </div>
          ) : (
            <div className="flex items-center space-x-2 text-slate-500">
              <ShieldCheck className="w-4 h-4 text-slate-600" />
              <span>Scanning for next AI Scalp Entry...</span>
            </div>
          )}
        </div>
      </div>

      {/* کانتینر اصلی چارت تریدینگ‌ویو */}
      <div className="flex-1 w-full bg-slate-900/30 border border-slate-800 rounded-xl overflow-hidden shadow-2xl relative min-h-[550px]">
        <div ref={chartContainerRef} className="absolute inset-0 w-full h-full" />
      </div>
    </div>
  );
}