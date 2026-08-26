// مسیر: rust_core/src/execution/mod.rs
use crate::config::TradingSettings;
use crate::domain::types::{CanonicalSignalPayload, OrderBookMetrics};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMakerOrder {
    pub order_id: String,
    pub signal_id: String,
    pub decision_id: String,
    pub symbol: String,
    pub is_buy: bool,
    pub limit_price: Decimal,
    pub quantity: Decimal,
    pub notional: Decimal,
    pub signal_price: Decimal,
    pub spread_at_signal: f64,
    pub signal_payload: CanonicalSignalPayload,
    pub placed_at: DateTime<Utc>,
    pub timeout_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalpPosition {
    pub position_id: String,
    pub signal_id: String,
    pub decision_id: String,
    pub order_id: String,
    pub fill_id: String,
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub is_buy: bool,
    pub take_profit_price: Decimal,
    pub stop_loss_price: Decimal,
    pub is_breakeven_active: bool,
    pub is_profit_locked: bool,
    pub is_open: bool,
    pub opened_at: DateTime<Utc>,
    pub fill_timestamp: DateTime<Utc>,
    pub max_holding_sec: i64,
    pub signal_payload: CanonicalSignalPayload,
    pub spread_at_fill: f64,
    pub entry_fee: Decimal,
    
    // فاکتورهای سنجش مسیر قیمت (Path Dependency)
    pub max_favorable_price: Decimal,
    pub max_adverse_price: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTradeRecord {
    // هویت و ردیابی رویدادها
    pub trade_id: String,
    pub signal_id: String,
    pub decision_id: String,
    pub order_id: String,
    pub fill_id: String,
    pub position_id: String,
    
    // متادیتا و نسخه‌ها
    pub strategy_version: String,
    pub model_version: String,
    pub feature_schema_version: String,
    
    // مشخصات بازار
    pub symbol: String,
    pub action: String,
    
    // زمان‌بندی دقیق به تفکیک میلی‌ثانیه (UTC)
    pub signal_timestamp: String,
    pub order_timestamp: String,
    pub fill_timestamp: String,
    pub exit_timestamp: String,
    pub duration_seconds: i64,
    
    // تاخیرهای سیستمی (Latency Metrics)
    pub signal_to_order_latency_ms: i64,
    pub order_to_fill_latency_ms: i64,
    
    // قیمت‌ها
    pub signal_price: Decimal,
    pub order_price: Decimal,
    pub fill_price: Decimal,
    pub entry_price: Decimal, // ✅ اضافه شد برای سازگاری کامل
    pub exit_price: Decimal,
    pub tp_price: Decimal,
    pub sl_price: Decimal,
    
    // حجم و ارزش
    pub quantity: Decimal,
    pub notional_usd: Decimal,
    
    // ساختار هزینه‌ها و اصطکاک صرافی
    pub entry_fee: Decimal,
    pub exit_fee: Decimal,
    pub total_fee: Decimal,
    pub fee_paid: Decimal, // ✅ اضافه شد برای سازگاری کامل
    pub spread_at_signal_bps: f64,
    pub spread_at_fill_bps: f64,
    pub slippage_bps: f64,
    
    // سود و زیان (تفکیک ناخالص و خالص)
    pub gross_pnl: Decimal,
    pub net_pnl: Decimal,
    pub pnl_percent: f64,
    pub pnl_bps: f64,
    
    // فاکتورهای پاتولوژی MFE / MAE
    pub mfe_bps: f64,
    pub mae_bps: f64,
    
    // ویژگی‌های سیگنال در لحظه ورود
    pub alpha_at_signal: f64,
    pub ofi_at_signal: f64,
    pub range_at_signal_bps: f64,
    pub price_drift_at_signal_bps: f64,
    pub volume_expanding_at_signal: bool,
    pub model_confidence: f64,
    
    // دلایل خروج
    pub exit_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRiskConfig {
    pub total_capital: Decimal,
    pub allocation_pct: Decimal,
    pub leverage: Decimal,
    pub min_notional_guard: Decimal,
    pub kill_switch: bool,
}

pub struct HighFrequencyScalpExecutor {
    positions: HashMap<String, ScalpPosition>,
    pending_orders: HashMap<String, PendingMakerOrder>,
    pub risk_config: DynamicRiskConfig,
    pub cash_balance: Decimal,
    pub total_margin_in_use: Decimal,
    maker_fee_ratio: Decimal,
    tp_ratio: Decimal,
    sl_ratio: Decimal,
    breakeven_ratio: Decimal,
    early_harvest_ratio: Decimal,
    max_holding_sec: i64,
    max_spread_bps: f64,
    maker_timeout_sec: i64,
}

impl HighFrequencyScalpExecutor {
    pub fn new(config: &TradingSettings) -> Self {
        let initial_cap = config.initial_capital;
        Self {
            positions: HashMap::new(),
            pending_orders: HashMap::new(),
            risk_config: DynamicRiskConfig {
                total_capital: initial_cap,
                allocation_pct: dec!(0.30),
                leverage: dec!(1.0),
                min_notional_guard: dec!(5.0),
                kill_switch: false,
            },
            cash_balance: initial_cap,
            total_margin_in_use: dec!(0.0),
            maker_fee_ratio: Decimal::from_f64_retain(config.maker_fee_bps / 10000.0).unwrap_or(dec!(0.0002)),
            tp_ratio: dec!(0.0014),
            sl_ratio: dec!(0.0014),
            breakeven_ratio: dec!(0.0008),
            early_harvest_ratio: dec!(0.0008),
            max_holding_sec: 120,
            max_spread_bps: config.max_spread_bps,
            maker_timeout_sec: 5,
        }
    }

    pub fn update_risk_config(&mut self, new_cap: Option<Decimal>, alloc_pct: Option<Decimal>, lev: Option<Decimal>, kill: Option<bool>) {
        if let Some(c) = new_cap {
            self.risk_config.total_capital = c;
            self.cash_balance = c;
            info!(capital = %c, "💵 Capital Balance Synchronized");
        }
        if let Some(a) = alloc_pct {
            self.risk_config.allocation_pct = a;
            info!(allocation = %a, "⚖️ Risk Allocation % Updated");
        }
        if let Some(l) = lev {
            self.risk_config.leverage = l;
            info!(leverage = %l, "⚡ Leverage Updated");
        }
        if let Some(k) = kill {
            self.risk_config.kill_switch = k;
            if k {
                warn!("🚨 EMERGENCY KILL-SWITCH ACTIVATED IN EXECUTION CORE!");
            }
        }
    }

    pub fn calculate_dynamic_notional(&self) -> Option<Decimal> {
        if self.risk_config.kill_switch {
            return None;
        }

        let free_cash = self.cash_balance.max(dec!(0.0));
        let notional = (free_cash * self.risk_config.allocation_pct * self.risk_config.leverage).round_dp(2);

        if notional < self.risk_config.min_notional_guard {
            return None;
        }

        Some(notional)
    }

    /// ایجاد و کاشت سفارش بر اساس سیگنال استاندارد دریافتی
    pub fn place_maker_order(
        &mut self,
        signal: &CanonicalSignalPayload,
        metrics: &OrderBookMetrics,
    ) -> Option<PendingMakerOrder> {
        if metrics.spread_bps > self.max_spread_bps || self.risk_config.kill_switch {
            return None;
        }

        let symbol = &signal.symbol;
        let is_buy = signal.action == "BUY";

        if let Some(pos) = self.positions.get(symbol) {
            if pos.is_open {
                return None;
            }
        }
        if self.pending_orders.contains_key(symbol) {
            return None;
        }

        let notional = self.calculate_dynamic_notional()?;
        let limit_price = (if is_buy { metrics.best_bid } else { metrics.best_ask }).round_dp(2);
        
        if limit_price <= dec!(0.0) {
            return None;
        }

        let size = (notional / limit_price).round_dp(4);
        let order_id = uuid::Uuid::new_v4().to_string();
        let decision_id = uuid::Uuid::new_v4().to_string();

        let order = PendingMakerOrder {
            order_id: order_id.clone(),
            signal_id: signal.signal_id.clone(),
            decision_id,
            symbol: symbol.clone(),
            is_buy,
            limit_price,
            quantity: size,
            notional,
            signal_price: signal.signal_price,
            spread_at_signal: metrics.spread_bps,
            signal_payload: signal.clone(),
            placed_at: Utc::now(),
            timeout_seconds: self.maker_timeout_sec,
        };

        self.pending_orders.insert(symbol.clone(), order.clone());
        info!(
            symbol = %symbol,
            order_id = %order_id,
            signal_id = %signal.signal_id,
            side = if is_buy { "BUY" } else { "SELL" },
            price = %limit_price,
            notional = %notional,
            "📥 Traceable Maker Order Placed"
        );
        Some(order)
    }

    /// بررسی پر شدن سفارش و ساخت پوزیشن با مقادیر اولیه MFE / MAE
    pub fn process_pending_orders_and_fills(
        &mut self,
        symbol: &str,
        trade_price: Decimal,
        current_spread_bps: f64,
    ) -> Option<ScalpPosition> {
        let order = self.pending_orders.get(symbol)?.clone();
        let now = Utc::now();
        let age = (now - order.placed_at).num_seconds();

        let is_filled = if order.is_buy {
            trade_price <= order.limit_price
        } else {
            trade_price >= order.limit_price
        };

        if is_filled {
            self.pending_orders.remove(symbol);
            let entry_fee = (order.notional * self.maker_fee_ratio).round_dp(4);

            let tp_price = if order.is_buy {
                (order.limit_price * (dec!(1.0) + self.tp_ratio)).round_dp(2)
            } else {
                (order.limit_price * (dec!(1.0) - self.tp_ratio)).round_dp(2)
            };

            let sl_price = if order.is_buy {
                (order.limit_price * (dec!(1.0) - self.sl_ratio)).round_dp(2)
            } else {
                (order.limit_price * (dec!(1.0) + self.sl_ratio)).round_dp(2)
            };

            let position = ScalpPosition {
                position_id: uuid::Uuid::new_v4().to_string(),
                signal_id: order.signal_id,
                decision_id: order.decision_id,
                order_id: order.order_id,
                fill_id: uuid::Uuid::new_v4().to_string(),
                symbol: symbol.to_string(),
                quantity: if order.is_buy { order.quantity } else { -order.quantity },
                entry_price: order.limit_price,
                is_buy: order.is_buy,
                take_profit_price: tp_price,
                stop_loss_price: sl_price,
                is_breakeven_active: false,
                is_profit_locked: false,
                is_open: true,
                opened_at: now,
                fill_timestamp: now,
                max_holding_sec: self.max_holding_sec,
                signal_payload: order.signal_payload,
                spread_at_fill: current_spread_bps,
                entry_fee,
                max_favorable_price: order.limit_price,
                max_adverse_price: order.limit_price,
            };

            self.cash_balance -= order.notional + entry_fee;
            self.total_margin_in_use += order.notional;
            self.positions.insert(symbol.to_string(), position.clone());

            info!(
                symbol = %symbol,
                position_id = %position.position_id,
                entry = %order.limit_price,
                tp = %tp_price,
                sl = %sl_price,
                "⚡ Scalp Position Opened & Linked"
            );
            return Some(position);
        }

        if age >= order.timeout_seconds {
            self.pending_orders.remove(symbol);
            info!(symbol = %symbol, "⌛ Stale Maker Order Cancelled");
        }

        None
    }

    /// به‌روزرسانی مداوم بالاترین و پایین‌ترین قیمت مشاهده‌شده برای محاسبه دقیق MFE / MAE
    pub fn update_path_dependency(&mut self, symbol: &str, current_price: Decimal) {
        if let Some(pos) = self.positions.get_mut(symbol) {
            if pos.is_open {
                if pos.is_buy {
                    if current_price > pos.max_favorable_price {
                        pos.max_favorable_price = current_price;
                    }
                    if current_price < pos.max_adverse_price {
                        pos.max_adverse_price = current_price;
                    }
                } else {
                    if current_price < pos.max_favorable_price {
                        pos.max_favorable_price = current_price;
                    }
                    if current_price > pos.max_adverse_price {
                        pos.max_adverse_price = current_price;
                    }
                }
            }
        }
    }

    /// ارزیابی خروج و تولید کامل‌ترین رکورد معاملاتی حسابرسی‌شده
    pub fn evaluate_open_positions(
        &mut self,
        symbol: &str,
        current_bid: Decimal,
        current_ask: Decimal,
    ) -> Option<ClosedTradeRecord> {
        let position = self.positions.get_mut(symbol)?;
        if !position.is_open {
            return None;
        }

        let now = Utc::now();
        let hold_duration = (now - position.opened_at).num_seconds();
        let mark_price = (if position.is_buy { current_bid } else { current_ask }).round_dp(2);

        if mark_price <= dec!(0.0) {
            return None;
        }

        // به‌روزرسانی نهایی MFE و MAE
        if position.is_buy {
            if mark_price > position.max_favorable_price { position.max_favorable_price = mark_price; }
            if mark_price < position.max_adverse_price { position.max_adverse_price = mark_price; }
        } else {
            if mark_price < position.max_favorable_price { position.max_favorable_price = mark_price; }
            if mark_price > position.max_adverse_price { position.max_adverse_price = mark_price; }
        }

        // ۱. قفل ریسک (Breakeven) پس از لمس سود اولیه
        if position.is_buy && mark_price >= position.entry_price * (dec!(1.0) + self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price + (position.entry_price * self.maker_fee_ratio * dec!(2))).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Breakeven Triggered: Zero-Risk Active");
        } else if !position.is_buy && mark_price <= position.entry_price * (dec!(1.0) - self.breakeven_ratio) && !position.is_breakeven_active {
            position.stop_loss_price = (position.entry_price - (position.entry_price * self.maker_fee_ratio * dec!(2))).round_dp(2);
            position.is_breakeven_active = true;
            info!(symbol = %symbol, "🛡️ Breakeven Triggered: Zero-Risk Active");
        }

        let hit_tp = if position.is_buy { mark_price >= position.take_profit_price } else { mark_price <= position.take_profit_price };
        let hit_sl = if position.is_buy { mark_price <= position.stop_loss_price } else { mark_price >= position.stop_loss_price };
        let hit_time = hold_duration >= position.max_holding_sec;

        let notional_current = mark_price * position.quantity.abs();
        let notional_entry = position.entry_price * position.quantity.abs();
        let exit_fee = (notional_current * self.maker_fee_ratio).round_dp(4);
        
        let current_raw_pnl = if position.is_buy {
            (mark_price - position.entry_price) * position.quantity.abs()
        } else {
            (position.entry_price - mark_price) * position.quantity.abs()
        };
        let current_net_pnl = current_raw_pnl - exit_fee;

        let dynamic_profit_harvest_target = notional_entry * self.early_harvest_ratio;
        let early_profit_take = hold_duration >= 15 && current_net_pnl >= dynamic_profit_harvest_target;

        let mut forced_scratch = false;
        let hit_scratch_time = hold_duration >= 60 && current_net_pnl >= dec!(0.0);

        let final_mark_price = if hit_scratch_time {
            forced_scratch = true;
            mark_price
        } else {
            mark_price
        };

        if hit_tp || hit_sl || early_profit_take || hit_time || forced_scratch {
            let gross_pnl = current_raw_pnl.round_dp(2);
            let total_fee = (position.entry_fee + exit_fee).round_dp(4);
            let net_pnl = (gross_pnl - total_fee).round_dp(2);
            
            let pnl_pct = if notional_entry > dec!(0.0) {
                let ratio = net_pnl / notional_entry;
                ratio.to_string().parse::<f64>().unwrap_or(0.0) * 100.0
            } else { 
                0.0 
            };
            let pnl_bps = pnl_pct * 100.0;

            // محاسبه دقیق MFE و MAE بر حسب Basis Points (BPS)
            let (mfe_bps, mae_bps) = if position.is_buy {
                let mfe = ((position.max_favorable_price - position.entry_price) / position.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                let mae = ((position.max_adverse_price - position.entry_price) / position.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                (mfe.max(0.0), mae.min(0.0))
            } else {
                let mfe = ((position.entry_price - position.max_favorable_price) / position.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                let mae = ((position.entry_price - position.max_adverse_price) / position.entry_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0;
                (mfe.max(0.0), mae.min(0.0))
            };

            // محاسبه اسلیپیج واقعی در لحظه ورود بر حسب BPS
            let slippage_bps = if position.is_buy {
                ((position.entry_price - position.signal_payload.signal_price) / position.signal_payload.signal_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0
            } else {
                ((position.signal_payload.signal_price - position.entry_price) / position.signal_payload.signal_price).to_string().parse::<f64>().unwrap_or(0.0) * 10000.0
            };

            // محاسبه تاخیرهای پردازش
            let signal_to_order_latency_ms = (position.opened_at - position.signal_payload.timestamp).num_milliseconds();
            let order_to_fill_latency_ms = (position.fill_timestamp - position.opened_at).num_milliseconds();

            position.is_open = false;
            self.total_margin_in_use -= notional_entry;
            self.cash_balance += notional_current + net_pnl;

            let reason = if hit_tp { 
                "MAKER_TAKE_PROFIT 🎯" 
            } else if early_profit_take { 
                "EARLY_PROFIT_HARVEST 💰" 
            } else if hit_sl { 
                "STOP_LOSS 🛑" 
            } else if forced_scratch {
                "SCRATCH_EXIT (NO LOSS) 🛡️"
            } else { 
                "TIME_EXPIRATION ⏱️" 
            };

            let record = ClosedTradeRecord {
                trade_id: uuid::Uuid::new_v4().to_string(),
                signal_id: position.signal_id.clone(),
                decision_id: position.decision_id.clone(),
                order_id: position.order_id.clone(),
                fill_id: position.fill_id.clone(),
                position_id: position.position_id.clone(),
                
                strategy_version: position.signal_payload.strategy_version.clone(),
                model_version: position.signal_payload.model_version.clone(),
                feature_schema_version: position.signal_payload.feature_schema_version.clone(),
                
                symbol: symbol.to_string(),
                action: if position.is_buy { "BUY".into() } else { "SELL".into() },
                
                signal_timestamp: position.signal_payload.timestamp.to_rfc3339(),
                order_timestamp: position.opened_at.to_rfc3339(),
                fill_timestamp: position.fill_timestamp.to_rfc3339(),
                exit_timestamp: now.to_rfc3339(),
                duration_seconds: hold_duration,
                
                signal_to_order_latency_ms,
                order_to_fill_latency_ms,
                
                signal_price: position.signal_payload.signal_price,
                order_price: position.entry_price,
                fill_price: position.entry_price,
                entry_price: position.entry_price, // ✅ مقداردهی شد
                exit_price: final_mark_price,
                tp_price: position.take_profit_price,
                sl_price: position.stop_loss_price,
                
                quantity: position.quantity.abs(),
                notional_usd: notional_entry,
                
                entry_fee: position.entry_fee,
                exit_fee,
                total_fee,
                fee_paid: total_fee, // ✅ مقداردهی شد
                spread_at_signal_bps: position.signal_payload.range_bps,
                spread_at_fill_bps: position.spread_at_fill,
                slippage_bps,
                
                gross_pnl,
                net_pnl,
                pnl_percent: (pnl_pct * 100.0).round() / 100.0,
                pnl_bps: (pnl_bps * 100.0).round() / 100.0,
                
                mfe_bps: (mfe_bps * 100.0).round() / 100.0,
                mae_bps: (mae_bps * 100.0).round() / 100.0,
                
                alpha_at_signal: position.signal_payload.alpha_score,
                ofi_at_signal: position.signal_payload.ofi,
                range_at_signal_bps: position.signal_payload.range_bps,
                price_drift_at_signal_bps: position.signal_payload.price_drift_bps,
                volume_expanding_at_signal: position.signal_payload.is_volume_expanding,
                model_confidence: position.signal_payload.probability,
                
                exit_reason: reason.to_string(),
            };

            info!(
                symbol = %symbol,
                trade_id = %record.trade_id,
                reason = %reason,
                net_pnl = %net_pnl,
                mfe_bps = %record.mfe_bps,
                mae_bps = %record.mae_bps,
                "📊 Trade Closed & Fully Attributed"
            );

            return Some(record);
        }

        None
    }

    pub fn get_cash(&self) -> Decimal {
        self.cash_balance
    }

    pub fn get_total_equity(&self) -> Decimal {
        self.cash_balance + self.total_margin_in_use
    }
}