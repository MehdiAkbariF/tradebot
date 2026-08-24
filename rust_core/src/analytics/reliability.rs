use std::collections::HashMap;

pub struct SourceReliabilityRegistry {
    scores: HashMap<String, f64>,
}

impl SourceReliabilityRegistry {
    pub fn new() -> Self {
        let mut scores = HashMap::new();
        scores.insert("reuters.com".into(), 0.95);
        scores.insert("bloomberg.com".into(), 0.95);
        scores.insert("coindesk.com".into(), 0.80);
        scores.insert("cointelegraph.com".into(), 0.75);
        
        Self { scores }
    }

    pub fn get_score(&self, source_id: &str) -> f64 {
        *self.scores.get(source_id).unwrap_or(&0.5)
    }

    pub fn update_score(&mut self, source_id: &str, accuracy_delta: f64) {
        let current = self.scores.entry(source_id.to_string()).or_insert(0.5);
        *current = (*current + accuracy_delta).clamp(0.0, 1.0);
    }
}