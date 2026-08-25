'use client';

import { useEffect, useRef, useState } from 'react';
import { 
  createChart, 
  IChartApi, 
  ISeriesApi, 
  CandlestickData, 
  SeriesMarker, 
  IPriceLine,
  UTCTimestamp,
  ColorType
} from 'lightweight-charts';
import { ShieldCheck, Zap } from 'lucide-react';
import { useTerminal, PositionUpdate } from '../context/TerminalContext';

export default function LiveTradeChart() {
  const terminal = useTerminal();
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const seriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  
  const [selectedSymbol, setSelectedSymbol] = useState<'BTCUSDT' | 'ETHUSDT'>('BTCUSDT');
  const [activeTrade, setActiveTrade] = useState<PositionUpdate | null>(null);

  const currentCandleRef = useRef<CandlestickData | null>(null);
  const markersRef = useRef<SeriesMarker<UTCTimestamp>[]>([]);
  const tpLineRef = useRef<IPriceLine | null>(null);
  const slLineRef = useRef<IPriceLine | null>(null);

  // ۱. راه‌اندازی چارت
  useEffect(() => {
    if (!chartContainerRef.current) return;

    const chart = createChart(chartContainerRef.current, {
      layout: {
        background: { type: ColorType.Solid, color: '#090d16' },
        textColor: '#94a3b8',
      },
      grid: {
        vertLines: { color: '#1e293b30' },
        horzLines: { color: '#1e293b30' },
      },
      crosshair: { mode: 1 },
      rightPriceScale: { borderColor: '#334155', autoScale: true },
      timeScale: { borderColor: '#334155', timeVisible: true, secondsVisible: true },
    });

    const candlestickSeries = (chart as any).addCandlestickSeries({
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

  // ۲. استریم تیک‌ها و ساخت کندل‌های ۵ ثانیه‌ای دقیق
  useEffect(() => {
    const tick = terminal.ticks[selectedSymbol];
    if (!tick) return;

    const price = parseFloat(tick.price);
    const tsMillis = tick.exchange_ts ? Number(tick.exchange_ts) : Date.now();
    const nowSec = Math.floor(tsMillis / 1000) as UTCTimestamp;
    const candleTime = (Math.floor(nowSec / 5) * 5) as UTCTimestamp;

    if (!currentCandleRef.current || (currentCandleRef.current.time as number) !== candleTime) {
      currentCandleRef.current = {
        time: candleTime,
        open: price,
        high: price,
        low: price,
        close: price,
      };
    } else {
      currentCandleRef.current.high = Math.max(currentCandleRef.current.high, price);
      currentCandleRef.current.low = Math.min(currentCandleRef.current.low, price);
      currentCandleRef.current.close = price;
    }

    if (seriesRef.current && currentCandleRef.current) {
      seriesRef.current.update(currentCandleRef.current);
    }
  }, [terminal.ticks, selectedSymbol]);

  // ۳. ترسیم نشانگرهای معامله با ضرایب اسکلپینگ استاندارد (TP: 0.08%, SL: 0.06%)
  useEffect(() => {
    const posData = terminal.lastPositionEvent;
    if (!posData || posData.symbol.toUpperCase() !== selectedSymbol) return;

    const nowSec = Math.floor(Date.now() / 1000) as UTCTimestamp;
    const markerTime = (Math.floor(nowSec / 5) * 5) as UTCTimestamp;

    if (posData.action === 'OPEN') {
      setActiveTrade(posData);
      const isBuy = posData.entry_price > 0 && !posData.action.includes('SELL');

      const newMarker: SeriesMarker<UTCTimestamp> = {
        time: markerTime,
        position: isBuy ? 'belowBar' : 'aboveBar',
        color: isBuy ? '#38bdf8' : '#f43f5e',
        shape: isBuy ? 'arrowUp' : 'arrowDown',
        text: `OPEN ${posData.symbol} @ $${posData.entry_price}`,
      };

      markersRef.current.push(newMarker);
      if (seriesRef.current && (seriesRef.current as any).setMarkers) {
        (seriesRef.current as any).setMarkers(markersRef.current);
      }

      if (seriesRef.current) {
        if (tpLineRef.current) seriesRef.current.removePriceLine(tpLineRef.current);
        if (slLineRef.current) seriesRef.current.removePriceLine(slLineRef.current);

        // تارگت‌های منطبق با نوسان واقعی میکرواستراکچر (8 bps TP و 6 bps SL)
        const tpPrice = isBuy ? posData.entry_price * 1.0008 : posData.entry_price * 0.9992;
        const slPrice = isBuy ? posData.entry_price * 0.9994 : posData.entry_price * 1.0006;

        tpLineRef.current = seriesRef.current.createPriceLine({
          price: parseFloat(tpPrice.toFixed(2)),
          color: '#10b981',
          lineWidth: 2,
          lineStyle: 2,
          axisLabelVisible: true,
          title: 'REALISTIC TP (0.08%)',
        });

        slLineRef.current = seriesRef.current.createPriceLine({
          price: parseFloat(slPrice.toFixed(2)),
          color: '#f43f5e',
          lineWidth: 2,
          lineStyle: 2,
          axisLabelVisible: true,
          title: 'TIGHT SL (0.06%)',
        });
      }
    } else if (posData.action === 'CLOSE') {
      setActiveTrade(null);

      if (seriesRef.current) {
        if (tpLineRef.current) seriesRef.current.removePriceLine(tpLineRef.current);
        if (slLineRef.current) seriesRef.current.removePriceLine(slLineRef.current);
      }

      const isProfit = (posData.pnl || 0) >= 0;
      const exitMarker: SeriesMarker<UTCTimestamp> = {
        time: markerTime,
        position: isProfit ? 'aboveBar' : 'belowBar',
        color: isProfit ? '#10b981' : '#f43f5e',
        shape: isProfit ? 'circle' : 'square',
        text: `CLOSED ${posData.reason || ''} | PnL: ${isProfit ? '+' : ''}$${posData.pnl?.toFixed(2)}`,
      };

      markersRef.current.push(exitMarker);
      if (seriesRef.current && (seriesRef.current as any).setMarkers) {
        (seriesRef.current as any).setMarkers(markersRef.current);
      }
    }
  }, [terminal.lastPositionEvent, selectedSymbol]);

  const currentPrice = terminal.ticks[selectedSymbol] ? parseFloat(terminal.ticks[selectedSymbol].price) : 0;

  return (
    <div className="flex flex-col h-full w-full bg-slate-950 p-6 space-y-4">
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
              5s Realtime Candles
            </span>
          </div>
        </div>

        <div className="flex items-center space-x-6 text-xs font-mono">
          {activeTrade ? (
            <div className="flex items-center space-x-3 bg-cyan-950/40 border border-cyan-500/30 px-3 py-1.5 rounded-lg">
              <Zap className="w-4 h-4 text-cyan-400 animate-pulse" />
              <span className="text-slate-300">
                Live Trade: <strong className="text-cyan-400">{activeTrade.action}</strong> @ ${activeTrade.entry_price}
              </span>
            </div>
          ) : (
            <div className="flex items-center space-x-2 text-slate-500">
              <ShieldCheck className="w-4 h-4 text-slate-600" />
              <span>Scanning High Imbalance & Spread Rebates...</span>
            </div>
          )}
        </div>
      </div>

      <div className="flex-1 w-full bg-slate-900/30 border border-slate-800 rounded-xl overflow-hidden shadow-2xl relative min-h-[550px]">
        <div ref={chartContainerRef} className="absolute inset-0 w-full h-full" />
      </div>
    </div>
  );
}