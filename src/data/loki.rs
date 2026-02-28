//! Loki log query client

use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;

use crate::node_marker::NodeHealth;

use super::{DataError, DataSource, NodeStatus};

/// Loki client configuration
#[derive(Clone)]
pub struct LokiConfig {
    pub base_url: String,
    /// LogQL query
    pub query: String,
    /// Label to use as node ID
    pub id_label: String,
    /// Time range in seconds (default: 300 = 5 minutes)
    pub range_seconds: u64,
    /// Error log patterns that indicate critical state
    pub critical_patterns: Vec<String>,
    /// Warning log patterns
    pub warning_patterns: Vec<String>,
}

impl Default for LokiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3100".to_string(),
            query: r#"{job="varlogs"}"#.to_string(),
            id_label: "instance".to_string(),
            range_seconds: 300,
            critical_patterns: vec!["error".to_string(), "fatal".to_string(), "panic".to_string()],
            warning_patterns: vec!["warn".to_string(), "warning".to_string()],
        }
    }
}

/// Loki query response
#[derive(Debug, Deserialize)]
struct LokiResponse {
    status: String,
    data: LokiData,
}

#[derive(Debug, Deserialize)]
struct LokiData {
    #[serde(rename = "resultType")]
    #[allow(dead_code)]
    result_type: String,
    result: Vec<LokiStream>,
}

#[derive(Debug, Deserialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<(String, String)>,
}

/// Loki data source client
pub struct LokiClient {
    pub config: LokiConfig,
    client: reqwest::Client,
}

impl LokiClient {
    pub fn new(config: LokiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn analyze_logs(&self, logs: &[(String, String)]) -> (NodeHealth, HashMap<String, f64>) {
        let mut critical_count = 0;
        let mut warning_count = 0;
        let total = logs.len();

        for (_, line) in logs {
            let lower = line.to_lowercase();

            if self
                .config
                .critical_patterns
                .iter()
                .any(|p| lower.contains(p))
            {
                critical_count += 1;
            } else if self
                .config
                .warning_patterns
                .iter()
                .any(|p| lower.contains(p))
            {
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
}

#[async_trait::async_trait]
impl DataSource for LokiClient {
    async fn fetch_nodes(&self) -> Result<Vec<NodeStatus>, DataError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let start = now - (self.config.range_seconds as u128 * 1_000_000_000);

        let url = format!(
            "{}/loki/api/v1/query_range?query={}&start={}&end={}",
            self.config.base_url,
            urlencoding::encode(&self.config.query),
            start,
            now
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body: LokiResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        if body.status != "success" {
            return Err(DataError::ParseError(format!(
                "Loki status: {}",
                body.status
            )));
        }

        let mut nodes = Vec::new();

        for stream in body.data.result {
            let id = stream
                .stream
                .get(&self.config.id_label)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());

            let lat = stream
                .stream
                .get("lat")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);
            let lon = stream
                .stream
                .get("lon")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);

            let (health, metrics) = self.analyze_logs(&stream.values);

            nodes.push(NodeStatus {
                id,
                lat,
                lon,
                health,
                metrics,
                last_updated: Instant::now(),
            });
        }

        Ok(nodes)
    }
}
