use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeAudit {
    pub source_ts: DateTime<Utc>,      // Generated at source exchange/publisher
    pub received_ts: DateTime<Utc>,    // Received by our network adapter
    pub processed_ts: DateTime<Utc>,   // Parsed and enriched by intelligence/NLP
    pub available_ts: DateTime<Utc>,   // Officially available to signal engine (Prevents Look-Ahead)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEvent {
    pub id: String,
    pub source_id: String,
    pub external_id: String,
    pub title: String,
    pub body: Option<String>,
    pub url: Option<String>,
    pub time_audit: TimeAudit,
    pub hash_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroEconomicEvent {
    pub id: String,
    pub event_name: String,
    pub country: String,
    pub currency: String,
    pub actual: Option<f64>,
    pub forecast: Option<f64>,
    pub previous: Option<f64>,
    pub surprise_z_score: Option<f64>,
    pub time_audit: TimeAudit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub id: String,
    pub event_type: String,
    pub primary_asset: String,
    pub relevance_score: f64,
    pub novelty_score: f64,
    pub summary: String,
    pub first_seen_ts: DateTime<Utc>,
    pub available_ts: DateTime<Utc>,
    pub related_news_ids: Vec<String>,
}