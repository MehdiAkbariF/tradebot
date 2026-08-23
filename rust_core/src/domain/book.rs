use super::types::{DepthDelta, OrderBookMetrics};
use crate::error::{AppError, Result};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::BTreeMap;
use std::cmp::Reverse;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub last_update_id: u64,
    // Bids sorted descending: Reverse(Price) ensures highest price is at .iter().next()
    pub bids: BTreeMap<Reverse<Decimal>, Decimal>,
    // Asks sorted ascending: lowest price is at .iter().next()
    pub asks: BTreeMap<Decimal, Decimal>,
    pub is_initialized: bool,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            last_update_id: 0,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            is_initialized: false,
        }
    }

    pub fn apply_snapshot(&mut self, last_update_id: u64, bids: Vec<(Decimal, Decimal)>, asks: Vec<(Decimal, Decimal)>) {
        self.bids.clear();
        self.asks.clear();

        for (price, qty) in bids {
            if qty > dec!(0) {
                self.bids.insert(Reverse(price), qty);
            }
        }

        for (price, qty) in asks {
            if qty > dec!(0) {
                self.asks.insert(price, qty);
            }
        }

        self.last_update_id = last_update_id;
        self.is_initialized = true;
    }

    pub fn apply_delta(&mut self, delta: &DepthDelta) -> Result<()> {
        if !self.is_initialized {
            return Ok(());
        }

        // Sequence Validation according to Binance documentation
        if delta.final_update_id <= self.last_update_id {
            // Drop old update
            return Ok(());
        }

        if delta.first_update_id > self.last_update_id + 1 {
            return Err(AppError::SequenceGap {
                symbol: self.symbol.clone(),
                expected: self.last_update_id + 1,
                received: delta.first_update_id,
            });
        }

        // Apply Bids updates
        for (price, qty) in &delta.bids {
            if *qty == dec!(0) {
                self.bids.remove(&Reverse(*price));
            } else {
                self.bids.insert(Reverse(*price), *qty);
            }
        }

        // Apply Asks updates
        for (price, qty) in &delta.asks {
            if *qty == dec!(0) {
                self.asks.remove(price);
            } else {
                self.asks.insert(*price, *qty);
            }
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

    pub fn compute_metrics(&self) -> Option<OrderBookMetrics> {
        let (best_bid_p, best_bid_q) = self.best_bid()?;
        let (best_ask_p, best_ask_q) = self.best_ask()?;

        if best_bid_p >= best_ask_q && best_ask_p <= best_bid_p {
            // Crossed book guard
            return None;
        }

        let mid_price = (best_bid_p + best_ask_p) / dec!(2);
        
        let spread = best_ask_p - best_bid_p;
        let spread_bps = if mid_price > dec!(0) {
            let ratio = spread / mid_price;
            let ratio_f64: f64 = ratio.to_string().parse().unwrap_or(0.0);
            ratio_f64 * 10000.0
        } else {
            0.0
        };

        // Micro-price calculation: (AskQty * BestBid + BidQty * BestAsk) / (BidQty + AskQty)
        let total_top_qty = best_bid_q + best_ask_q;
        let micro_price = if total_top_qty > dec!(0) {
            (best_ask_q * best_bid_p + best_bid_q * best_ask_p) / total_top_qty
        } else {
            mid_price
        };

        // Imbalance over top 10 levels
        let bid_volume_10: Decimal = self.bids.values().take(10).sum();
        let ask_volume_10: Decimal = self.asks.values().take(10).sum();
        let total_vol = bid_volume_10 + ask_volume_10;
        
        let imbalance = if total_vol > dec!(0) {
            let diff = bid_volume_10 - ask_volume_10;
            let imb_ratio = diff / total_vol;
            imb_ratio.to_string().parse().unwrap_or(0.0)
        } else {
            0.0
        };

        Some(OrderBookMetrics {
            symbol: self.symbol.clone(),
            best_bid: best_bid_p,
            best_ask: best_ask_p,
            spread_bps,
            mid_price,
            micro_price,
            imbalance_top10: imbalance,
            timestamp: Utc::now(),
        })
    }
}