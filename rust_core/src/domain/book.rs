// مسیر: rust_core/src/domain/book.rs
use super::types::{DepthDelta, MicrosecondAudit, OrderBookMetrics};
use crate::error::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, VecDeque};

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub last_update_id: u64,
    pub bids: BTreeMap<Reverse<Decimal>, Decimal>,
    pub asks: BTreeMap<Decimal, Decimal>,
    pub is_initialized: bool,
    
    prev_bid_levels: HashMap<Decimal, Decimal>,
    prev_ask_levels: HashMap<Decimal, Decimal>,
    
    l2_ofi_history_60s: VecDeque<(DateTime<Utc>, f64)>,
    mid_price_history_60s: VecDeque<(DateTime<Utc>, Decimal)>,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            last_update_id: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            is_initialized: false,
            prev_bid_levels: HashMap::new(),
            prev_ask_levels: HashMap::new(),
            l2_ofi_history_60s: VecDeque::with_capacity(300),
            mid_price_history_60s: VecDeque::with_capacity(300),
        }
    }

    pub fn apply_snapshot(&mut self, last_update_id: u64, bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) {
        self.bids.clear();
        self.asks.clear();
        self.prev_bid_levels.clear();
        self.prev_ask_levels.clear();

        for (p, q) in bids {
            if q > dec!(0) {
                self.bids.insert(Reverse(p), q);
                self.prev_bid_levels.insert(p, q);
            }
        }
        for (p, q) in asks {
            if q > dec!(0) {
                self.asks.insert(p, q);
                self.prev_ask_levels.insert(p, q);
            }
        }

        self.last_update_id = last_update_id;
        self.is_initialized = true;
    }

    pub fn apply_delta(&mut self, delta: &DepthDelta) -> Result<()> {
        // ۱. اگر اولین پیام دریافتی است، به عنوان اسنپ‌شات اولیه پذیرفته شود
        if self.last_update_id == 0 || !self.is_initialized {
            self.apply_snapshot(delta.final_update_id, delta.bids.clone(), delta.asks.clone());
            return Ok(());
        }

        if delta.final_update_id <= self.last_update_id { return Ok(()); }

        for (price, qty) in &delta.bids {
            if *qty == dec!(0) { self.bids.remove(&Reverse(*price)); }
            else { self.bids.insert(Reverse(*price), *qty); }
        }

        for (price, qty) in &delta.asks {
            if *qty == dec!(0) { self.asks.remove(price); }
            else { self.asks.insert(*price, *qty); }
        }

        self.last_update_id = delta.final_update_id;
        Ok(())
    }

    pub fn best_bid(&self) -> Option<(Decimal, Decimal)> {
        self.bids.iter().next().map(|(Reverse(p), q)| (*p, *q))
    }

    pub fn best_ask(&self) -> Option<(Decimal, Decimal)> {
        self.asks.iter().next().map(|(p, q)| (*p, *q))
    }

    pub fn get_top_k_levels(&self, k: usize) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let top_bids: Vec<(Decimal, Decimal)> = self.bids.iter().take(k).map(|(Reverse(p), q)| (*p, *q)).collect();
        let top_asks: Vec<(Decimal, Decimal)> = self.asks.iter().take(k).map(|(p, q)| (*p, *q)).collect();
        (top_bids, top_asks)
    }

    pub fn compute_metrics(&mut self, audit: MicrosecondAudit) -> Option<OrderBookMetrics> {
        let (bb_p, bb_q) = self.best_bid()?;
        let (ba_p, ba_q) = self.best_ask()?;
        if bb_p >= ba_p { return None; }

        let mid_price = (bb_p + ba_p) / dec!(2.0);
        let spread = ba_p - bb_p;
        let spread_bps = (spread / mid_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;

        let micro_price = if (bb_q + ba_q) > dec!(0) {
            (ba_q * bb_p + bb_q * ba_p) / (bb_q + ba_q)
        } else {
            mid_price
        };

        // محاسبه OFI چندسطحی بر مبنای هویت قیمت (L2 Multi-Level OFI)
        let (current_top_bids, current_top_asks) = self.get_top_k_levels(5);
        let mut current_bid_map = HashMap::new();
        for &(p, q) in &current_top_bids { current_bid_map.insert(p, q); }
        let mut current_ask_map = HashMap::new();
        for &(p, q) in &current_top_asks { current_ask_map.insert(p, q); }

        let mut l2_ofi_step = 0.0;

        for (&p, &q_curr) in &current_bid_map {
            let q_prev = *self.prev_bid_levels.get(&p).unwrap_or(&dec!(0));
            let delta_q = q_curr - q_prev;
            let level_idx = current_top_bids.iter().position(|&(lp, _)| lp == p).unwrap_or(0);
            let weight = 1.0 / (level_idx as f64 + 1.0);
            l2_ofi_step += weight * delta_q.to_string().parse::<f64>().unwrap_or(0.0);
        }

        for (&p, &q_curr) in &current_ask_map {
            let q_prev = *self.prev_ask_levels.get(&p).unwrap_or(&dec!(0));
            let delta_q = q_curr - q_prev;
            let level_idx = current_top_asks.iter().position(|&(lp, _)| lp == p).unwrap_or(0);
            let weight = 1.0 / (level_idx as f64 + 1.0);
            l2_ofi_step -= weight * delta_q.to_string().parse::<f64>().unwrap_or(0.0);
        }

        self.prev_bid_levels = current_bid_map;
        self.prev_ask_levels = current_ask_map;

        let event_time = audit.exchange_ts;

        while let Some(front) = self.l2_ofi_history_60s.front() {
            if event_time.signed_duration_since(front.0).num_seconds() > 60 {
                self.l2_ofi_history_60s.pop_front();
            } else { break; }
        }
        while let Some(front) = self.mid_price_history_60s.front() {
            if event_time.signed_duration_since(front.0).num_seconds() > 60 {
                self.mid_price_history_60s.pop_front();
            } else { break; }
        }

        // محاسبه Z-score آماری واقعی
        let ofi_zscore = if self.l2_ofi_history_60s.len() >= 10 {
            let hist: Vec<f64> = self.l2_ofi_history_60s.iter().map(|x| x.1).collect();
            let mean = hist.iter().sum::<f64>() / hist.len() as f64;
            let var = hist.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / hist.len() as f64;
            let std = var.sqrt();

            if std > 1e-4 {
                ((l2_ofi_step - mean) / std).clamp(-3.0, 3.0)
            } else if l2_ofi_step.abs() > 0.001 {
                if l2_ofi_step > 0.0 { 1.5 } else { -1.5 }
            } else {
                0.0
            }
        } else if l2_ofi_step.abs() > 0.001 {
            if l2_ofi_step > 0.0 { 1.2 } else { -1.2 }
        } else {
            0.0
        };

        self.l2_ofi_history_60s.push_back((event_time, l2_ofi_step));
        self.mid_price_history_60s.push_back((event_time, mid_price));

        let vol_bps = if self.mid_price_history_60s.len() > 2 {
            let mut sum_sq = 0.0;
            let slice = self.mid_price_history_60s.make_contiguous();
            for w in slice.windows(2) {
                let r = ((w[1].1 - w[0].1) / w[0].1).to_string().parse::<f64>().unwrap_or(0.0);
                sum_sq += r * r;
            }
            (sum_sq / self.mid_price_history_60s.len() as f64).sqrt() * 10000.0
        } else {
            1.0
        };

        let bid_top10: Decimal = self.bids.values().take(10).sum();
        let ask_top10: Decimal = self.asks.values().take(10).sum();
        let imbalance = if (bid_top10 + ask_top10) > dec!(0) {
            ((bid_top10 - ask_top10) / (bid_top10 + ask_top10)).to_string().parse::<f64>().unwrap_or(0.0)
        } else { 0.0 };

        Some(OrderBookMetrics {
            symbol: self.symbol.clone(),
            best_bid: bb_p,
            best_ask: ba_p,
            spread_bps,
            mid_price,
            micro_price,
            ofi: l2_ofi_step,
            ofi_zscore,
            book_imbalance_top10: imbalance,
            imbalance_top10: imbalance,
            realized_vol_60s_bps: vol_bps,
            timestamp: event_time,
            audit,
        })
    }
}