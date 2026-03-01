//! NodeStatus struct and buffer resource

use std::collections::HashMap;
use std::time::Instant;

use bevy::prelude::*;

use crate::node_marker::NodeHealth;

/// Status data for a monitored node
#[derive(Clone, Debug)]
pub struct NodeStatus {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub health: NodeHealth,
    pub metrics: HashMap<String, f64>,
    pub last_updated: Instant,
}

impl NodeStatus {
    pub fn new(id: impl Into<String>, lat: f64, lon: f64) -> Self {
        Self {
            id: id.into(),
            lat,
            lon,
            health: NodeHealth::Healthy,
            metrics: HashMap::new(),
            last_updated: Instant::now(),
        }
    }

    pub fn with_health(mut self, health: NodeHealth) -> Self {
        self.health = health;
        self
    }

    pub fn with_metric(mut self, key: impl Into<String>, value: f64) -> Self {
        self.metrics.insert(key.into(), value);
        self
    }
}

/// Resource holding the current status of all nodes
#[derive(Resource, Default)]
pub struct NodeStatusBuffer {
    pub nodes: HashMap<String, NodeStatus>,
}

impl NodeStatusBuffer {
    pub fn update(&mut self, status: NodeStatus) {
        self.nodes.insert(status.id.clone(), status);
    }

    pub fn get(&self, id: &str) -> Option<&NodeStatus> {
        self.nodes.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &NodeStatus> {
        self.nodes.values()
    }
}

/// Health classification thresholds
///
/// Values at or below `critical` are Critical, values at or below `warning` are Warning,
/// and values above `warning` are Healthy. Requires `warning > critical`.
#[derive(Clone, Debug)]
pub struct HealthThresholds {
    pub warning: f64,
    pub critical: f64,
}

impl Default for HealthThresholds {
    fn default() -> Self {
        Self {
            warning: 0.5,
            critical: 0.0,
        }
    }
}

impl HealthThresholds {
    pub fn classify(&self, value: f64) -> NodeHealth {
        if value <= self.critical {
            NodeHealth::Critical
        } else if value <= self.warning {
            NodeHealth::Warning
        } else {
            NodeHealth::Healthy
        }
    }
}

/// Configuration for log-based health analysis
#[derive(Clone, Debug)]
pub struct LogAnalysisConfig {
    pub critical_patterns: Vec<String>,
    pub warning_patterns: Vec<String>,
}

impl Default for LogAnalysisConfig {
    fn default() -> Self {
        Self {
            critical_patterns: vec!["error".to_string(), "fatal".to_string(), "panic".to_string()],
            warning_patterns: vec!["warn".to_string(), "warning".to_string()],
        }
    }
}

/// Analyze log lines and classify health based on pattern matches.
/// Returns (health, metrics) where metrics contains log_count, error_count, warning_count.
pub fn analyze_logs(
    logs: &[(String, String)],
    config: &LogAnalysisConfig,
) -> (NodeHealth, HashMap<String, f64>) {
    let mut critical_count = 0;
    let mut warning_count = 0;
    let total = logs.len();

    for (_, line) in logs {
        let lower = line.to_lowercase();
        if config.critical_patterns.iter().any(|p| lower.contains(p)) {
            critical_count += 1;
        } else if config.warning_patterns.iter().any(|p| lower.contains(p)) {
            warning_count += 1;
        }
    }

    let health = if critical_count > 0 {
        NodeHealth::Critical
    } else if warning_count > 0 {
        NodeHealth::Warning
    } else {
        NodeHealth::Healthy
    };

    let mut metrics = HashMap::new();
    metrics.insert("log_count".to_string(), total as f64);
    metrics.insert("error_count".to_string(), critical_count as f64);
    metrics.insert("warning_count".to_string(), warning_count as f64);

    (health, metrics)
}

/// Error type for data source operations
#[derive(Debug, Clone)]
pub enum DataError {
    NetworkError(String),
    ParseError(String),
    AuthError(String),
    NotFound(String),
}

impl std::fmt::Display for DataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            DataError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            DataError::AuthError(msg) => write!(f, "Auth error: {}", msg),
            DataError::NotFound(msg) => write!(f, "Not found: {}", msg),
        }
    }
}

impl std::error::Error for DataError {}

/// Trait for data sources that provide NodeStatus updates
#[async_trait::async_trait]
pub trait DataSource: Send + Sync {
    async fn fetch_nodes(&self) -> Result<Vec<NodeStatus>, DataError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_logs_healthy_when_no_patterns() {
        let patterns = LogAnalysisConfig {
            critical_patterns: vec!["error".to_string(), "fatal".to_string()],
            warning_patterns: vec!["warn".to_string()],
        };
        let logs = vec![
            ("1".to_string(), "all systems normal".to_string()),
            ("2".to_string(), "request completed".to_string()),
        ];
        let (health, metrics) = analyze_logs(&logs, &patterns);
        assert_eq!(health, NodeHealth::Healthy);
        assert_eq!(metrics["log_count"], 2.0);
        assert_eq!(metrics["error_count"], 0.0);
        assert_eq!(metrics["warning_count"], 0.0);
    }

    #[test]
    fn analyze_logs_critical_when_error_found() {
        let patterns = LogAnalysisConfig {
            critical_patterns: vec!["error".to_string(), "fatal".to_string()],
            warning_patterns: vec!["warn".to_string()],
        };
        let logs = vec![
            ("1".to_string(), "fatal crash detected".to_string()),
        ];
        let (health, metrics) = analyze_logs(&logs, &patterns);
        assert_eq!(health, NodeHealth::Critical);
        assert_eq!(metrics["error_count"], 1.0);
    }

    #[test]
    fn analyze_logs_warning_when_warn_found() {
        let patterns = LogAnalysisConfig {
            critical_patterns: vec!["error".to_string()],
            warning_patterns: vec!["warn".to_string()],
        };
        let logs = vec![
            ("1".to_string(), "disk usage warning threshold".to_string()),
        ];
        let (health, metrics) = analyze_logs(&logs, &patterns);
        assert_eq!(health, NodeHealth::Warning);
        assert_eq!(metrics["warning_count"], 1.0);
    }

    #[test]
    fn analyze_logs_empty_input() {
        let patterns = LogAnalysisConfig::default();
        let logs: Vec<(String, String)> = vec![];
        let (health, _metrics) = analyze_logs(&logs, &patterns);
        assert_eq!(health, NodeHealth::Healthy);
    }

    #[test]
    fn classify_health_thresholds() {
        let t = HealthThresholds { warning: 0.5, critical: 0.0 };
        assert_eq!(t.classify(1.0), NodeHealth::Healthy);
        assert_eq!(t.classify(0.5), NodeHealth::Warning);
        assert_eq!(t.classify(0.3), NodeHealth::Warning);
        assert_eq!(t.classify(0.0), NodeHealth::Critical);
        assert_eq!(t.classify(-1.0), NodeHealth::Critical);
    }
}
