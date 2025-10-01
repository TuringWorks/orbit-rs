use std::collections::HashMap;

/// Extensions for working with metrics
pub struct MetricsExt;

impl MetricsExt {
    /// Increment a counter with labels
    pub fn increment_counter(name: String, labels: HashMap<String, String>) {
        // For now, just use the basic metrics without labels to avoid lifetime issues
        // This can be enhanced later with proper metrics recorder setup
        metrics::counter!(name.clone()).increment(1);
        
        // Add labels as additional separate metrics for context
        for (key, value) in labels {
            let metric_name = format!("{}.{}", name, key);
            metrics::counter!(metric_name, "value" => value).increment(1);
        }
    }

    /// Set a gauge value with labels
    pub fn set_gauge(name: String, value: f64, labels: HashMap<String, String>) {
        metrics::gauge!(name.clone()).set(value);
        
        // Add context through separate labeled metrics
        for (key, label_value) in labels {
            let metric_name = format!("{}.{}", name, key);
            metrics::gauge!(metric_name, "value" => label_value).set(value);
        }
    }

    /// Record a histogram value with labels
    pub fn record_histogram(name: String, value: f64, labels: HashMap<String, String>) {
        metrics::histogram!(name.clone()).record(value);
        
        // Add context through separate labeled metrics
        for (key, label_value) in labels {
            let metric_name = format!("{}.{}", name, key);
            metrics::histogram!(metric_name, "value" => label_value).record(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_ext() {
        let mut labels = HashMap::new();
        labels.insert("environment".to_string(), "test".to_string());
        labels.insert("service".to_string(), "orbit".to_string());
        
        // These would normally be tested with a metrics recorder setup
        // For now, just test that the functions don't panic
        MetricsExt::increment_counter("test_counter".to_string(), labels.clone());
        MetricsExt::set_gauge("test_gauge".to_string(), 42.0, labels.clone());
        MetricsExt::record_histogram("test_histogram".to_string(), 1.0, labels);
    }
}
