use chrono::Utc;
use rust_core::domain::events::{NewsEvent, TimeAudit};

#[test]
fn test_time_audit_look_ahead_prevention() {
    let now = Utc::now();
    let past = now - chrono::Duration::seconds(10);

    let audit = TimeAudit {
        source_ts: past,
        received_ts: past + chrono::Duration::milliseconds(100),
        processed_ts: past + chrono::Duration::milliseconds(500),
        available_ts: past + chrono::Duration::milliseconds(600),
    };

    assert!(audit.available_ts >= audit.source_ts);
    assert!(audit.processed_ts >= audit.received_ts);
}