// مسیر: rust_core/src/market_data/normalizer.rs
use crate::domain::types::{DepthDelta, MicrosecondAudit, TradeTick};
use crate::error::{AppError, Result};
use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use std::str::FromStr;

pub struct BinanceNormalizer;

impl BinanceNormalizer {
    pub fn parse_trade(val: &Value, received_ts: DateTime<Utc>) -> Result<TradeTick> {
        let symbol = val["s"].as_str().ok_or_else(|| AppError::Protocol("Missing s field".into()))?.to_string();
        let trade_id = val["t"].as_u64().ok_or_else(|| AppError::Protocol("Missing t field".into()))?;
        
        let price_str = val["p"].as_str().ok_or_else(|| AppError::Protocol("Missing p field".into()))?;
        let price = Decimal::from_str(price_str).map_err(|e| AppError::Protocol(format!("Invalid price: {}", e)))?;

        let qty_str = val["q"].as_str().ok_or_else(|| AppError::Protocol("Missing q field".into()))?;
        let quantity = Decimal::from_str(qty_str).map_err(|e| AppError::Protocol(format!("Invalid quantity: {}", e)))?;

        let is_buyer_maker = val["m"].as_bool().unwrap_or(false);
        let time_millis = val["T"].as_i64().ok_or_else(|| AppError::Protocol("Missing T field".into()))?;
        let exchange_ts = Utc.timestamp_millis_opt(time_millis).single().unwrap_or(received_ts);

        let audit = MicrosecondAudit {
            exchange_ts,
            receive_ts: received_ts,
            process_ts: Utc::now(),
            decision_ts: None,
            submit_ts: None,
            fill_ts: None,
        };

        Ok(TradeTick {
            symbol,
            trade_id,
            price,
            quantity,
            is_buyer_maker,
            exchange_ts,
            received_ts,
            audit,
        })
    }

    pub fn parse_depth_delta(val: &Value, received_ts: DateTime<Utc>) -> Result<DepthDelta> {
        let symbol = val["s"].as_str().ok_or_else(|| AppError::Protocol("Missing s in depth".into()))?.to_string();
        let first_update_id = val["U"].as_u64().ok_or_else(|| AppError::Protocol("Missing U in depth".into()))?;
        let final_update_id = val["u"].as_u64().ok_or_else(|| AppError::Protocol("Missing u in depth".into()))?;
        
        let time_millis = val["E"].as_i64().unwrap_or(received_ts.timestamp_millis());
        let exchange_ts = Utc.timestamp_millis_opt(time_millis).single().unwrap_or(received_ts);

        let bids = Self::parse_levels(&val["b"])?;
        let asks = Self::parse_levels(&val["a"])?;

        let audit = MicrosecondAudit {
            exchange_ts,
            receive_ts: received_ts,
            process_ts: Utc::now(),
            decision_ts: None,
            submit_ts: None,
            fill_ts: None,
        };

        Ok(DepthDelta {
            symbol,
            first_update_id,
            final_update_id,
            bids,
            asks,
            exchange_ts,
            received_ts,
            audit,
        })
    }

    pub fn parse_levels(val: &Value) -> Result<Vec<(Decimal, Decimal)>> {
        let mut levels = Vec::new();
        if let Some(arr) = val.as_array() {
            for item in arr {
                if let (Some(p_str), Some(q_str)) = (item[0].as_str(), item[1].as_str()) {
                    let p = Decimal::from_str(p_str).map_err(|e| AppError::Protocol(e.to_string()))?;
                    let q = Decimal::from_str(q_str).map_err(|e| AppError::Protocol(e.to_string()))?;
                    levels.push((p, q));
                }
            }
        }
        Ok(levels)
    }
}