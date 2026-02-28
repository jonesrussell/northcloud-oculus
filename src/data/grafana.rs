//! Grafana API client

use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;

use crate::node_marker::NodeHealth;

use super::{DataError, DataSource, NodeStatus};

/// Grafana client configuration
#[derive(Clone)]
pub struct GrafanaConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub datasource_uid: String,
    /// PromQL query to execute via Grafana datasource proxy
    pub query: String,
    /// Label to use as node ID
    pub id_label: String,
    /// Health thresholds
    pub warning_threshold: f64,
    pub critical_threshold: f64,
}

impl Default for GrafanaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            api_key: None,
            datasource_uid: "prometheus".to_string(),
            query: "up".to_string(),
            id_label: "instance".to_string(),
            warning_threshold: 0.5,
            critical_threshold: 0.0,
        }
    }
}

/// Grafana datasource query response
#[derive(Debug, Deserialize)]
struct GrafanaQueryResponse {
    results: HashMap<String, GrafanaQueryResult>,
}

#[derive(Debug, Deserialize)]
struct GrafanaQueryResult {
    frames: Vec<GrafanaFrame>,
}

#[derive(Debug, Deserialize)]
struct GrafanaFrame {
    schema: GrafanaSchema,
    data: GrafanaFrameData,
}

#[derive(Debug, Deserialize)]
struct GrafanaSchema {
    fields: Vec<GrafanaField>,
}

#[derive(Debug, Deserialize)]
struct GrafanaField {
    name: String,
    labels: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct GrafanaFrameData {
    values: Vec<Vec<serde_json::Value>>,
}

/// Grafana data source client
pub struct GrafanaClient {
    pub config: GrafanaConfig,
    client: reqwest::Client,
}

impl GrafanaClient {
    pub fn new(config: GrafanaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn parse_health(&self, value: f64) -> NodeHealth {
        if value <= self.config.critical_threshold {
            NodeHealth::Critical
        } else if value <= self.config.warning_threshold {
            NodeHealth::Warning
        } else {
            NodeHealth::Healthy
        }
    }
}

#[async_trait::async_trait]
impl DataSource for GrafanaClient {
    async fn fetch_nodes(&self) -> Result<Vec<NodeStatus>, DataError> {
        let url = format!("{}/api/ds/query", self.config.base_url);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": {
                    "uid": self.config.datasource_uid
                },
                "expr": self.config.query,
                "instant": true
            }],
            "from": (now - 60000).to_string(),
            "to": now.to_string()
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DataError::AuthError("Invalid API key".to_string()));
        }

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body: GrafanaQueryResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        let mut nodes = Vec::new();

        if let Some(result) = body.results.get("A") {
            for frame in &result.frames {
                for (i, field) in frame.schema.fields.iter().enumerate() {
                    if field.name == "Value" {
                        if let Some(labels) = &field.labels {
                            let id = labels
                                .get(&self.config.id_label)
                                .cloned()
                                .unwrap_or_else(|| format!("node-{}", i));

                            let lat = labels
                                .get("lat")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.0);
                            let lon = labels
                                .get("lon")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(0.0);

                            let value = frame
                                .data
                                .values
                                .get(i)
                                .and_then(|vals| vals.last())
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0);

                            let health = self.parse_health(value);

                            let mut status = NodeStatus {
                                id,
                                lat,
                                lon,
                                health,
                                metrics: HashMap::new(),
                                last_updated: Instant::now(),
                            };

                            status.metrics.insert("value".to_string(), value);
                            nodes.push(status);
                        }
                    }
                }
            }
        }

        Ok(nodes)
    }
}
