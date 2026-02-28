//! Prometheus polling client

use bevy::log::warn;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;

use super::{DataError, DataSource, HealthThresholds, NodeStatus};

/// Prometheus client configuration
#[derive(Clone)]
pub struct PrometheusConfig {
    pub base_url: String,
    /// PromQL query to execute. Result labels are used to extract node ID (via id_label),
    /// and optionally latitude/longitude (via lat_label, lon_label).
    pub query: String,
    /// Label to use as node ID (default: "instance")
    pub id_label: String,
    /// Label containing latitude (optional)
    pub lat_label: Option<String>,
    /// Label containing longitude (optional)
    pub lon_label: Option<String>,
    /// Health classification thresholds
    pub thresholds: HealthThresholds,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:9090".to_string(),
            query: "up".to_string(),
            id_label: "instance".to_string(),
            lat_label: None,
            lon_label: None,
            thresholds: HealthThresholds::default(),
        }
    }
}

/// Prometheus query response structure
#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    #[serde(rename = "resultType")]
    #[allow(dead_code)]
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    metric: HashMap<String, String>,
    value: (f64, String),
}

/// Prometheus data source
pub struct PrometheusClient {
    pub config: PrometheusConfig,
    client: reqwest::Client,
}

impl PrometheusClient {
    pub fn new(config: PrometheusConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

#[async_trait::async_trait]
impl DataSource for PrometheusClient {
    async fn fetch_nodes(&self) -> Result<Vec<NodeStatus>, DataError> {
        let url = format!(
            "{}/api/v1/query?query={}",
            self.config.base_url,
            urlencoding::encode(&self.config.query)
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "Prometheus query failed: HTTP {}",
                response.status()
            )));
        }

        let body: PrometheusResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        if body.status != "success" {
            return Err(DataError::ParseError(format!(
                "Prometheus status: {}",
                body.status
            )));
        }

        let mut nodes = Vec::new();

        for result in body.data.result {
            let id = result
                .metric
                .get(&self.config.id_label)
                .cloned()
                .unwrap_or_else(|| {
                    warn!("Prometheus: missing '{}' label, using fallback ID", self.config.id_label);
                    "unknown".to_string()
                });

            let lat = self
                .config
                .lat_label
                .as_ref()
                .and_then(|l| result.metric.get(l))
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| {
                    if self.config.lat_label.is_some() {
                        warn!("Prometheus node {id}: missing or invalid lat label, defaulting to 0.0");
                    }
                    0.0
                });

            let lon = self
                .config
                .lon_label
                .as_ref()
                .and_then(|l| result.metric.get(l))
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| {
                    if self.config.lon_label.is_some() {
                        warn!("Prometheus node {id}: missing or invalid lon label, defaulting to 0.0");
                    }
                    0.0
                });

            let value: f64 = result
                .value
                .1
                .parse()
                .map_err(|_| DataError::ParseError("Invalid metric value".to_string()))?;

            let health = self.config.thresholds.classify(value);

            let mut status = NodeStatus {
                id,
                lat,
                lon,
                health,
                metrics: HashMap::new(),
                last_updated: Instant::now(),
            };

            status.metrics.insert("value".to_string(), value);

            for (key, val) in &result.metric {
                if key != &self.config.id_label {
                    if let Ok(v) = val.parse::<f64>() {
                        status.metrics.insert(key.clone(), v);
                    }
                }
            }

            nodes.push(status);
        }

        Ok(nodes)
    }
}
