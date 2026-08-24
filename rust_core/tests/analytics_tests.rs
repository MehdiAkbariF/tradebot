use rust_core::analytics::regime::{MarketRegime, RegimeDetector};
use rust_core::analytics::cross_asset::CrossAssetAnalyzer;
use rust_core::analytics::reliability::SourceReliabilityRegistry;

#[test]
fn test_market_regime_detection() {
    let detector = RegimeDetector::new(0.02); // 2% vol threshold
    
    let bullish_returns = vec![0.005, 0.006, 0.004, 0.007];
    let regime = detector.detect_regime(&bullish_returns, 1.2);
    
    assert_eq!(regime, MarketRegime::BullishTrend);
}

#[test]
fn test_cross_asset_lead_lag() {
    let leader = vec![0.01, 0.02, -0.01, 0.005, 0.03];
    let follower = vec![0.0, 0.01, 0.02, -0.01, 0.005];

    let (lag, corr) = CrossAssetAnalyzer::compute_lead_lag(&leader, &follower, 2);
    
    assert_eq!(lag, 1);
    assert!(corr > 0.5);
}

#[test]
fn test_source_reliability_registry() {
    let mut registry = SourceReliabilityRegistry::new();
    
    let reuters_score = registry.get_score("reuters.com");
    assert_eq!(reuters_score, 0.95);

    let unknown_score = registry.get_score("unknown-blog.xyz");
    assert_eq!(unknown_score, 0.5);
}