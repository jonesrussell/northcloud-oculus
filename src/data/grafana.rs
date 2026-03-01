//! Grafana datasource proxy client for Prometheus metric queries and Loki log queries

use bevy::log::warn;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Instant;

use super::{DataError, HealthThresholds, LogAnalysisConfig, NodeStatus};

/// A raw log entry from Loki (before health analysis)
#[derive(Clone, Debug)]
pub struct RawLogEntry {
    pub timestamp: Instant,
    pub source: String,
    pub message: String,
    pub labels: HashMap<String, String>,
}

/// Grafana connection configuration (base URL and authentication)
#[derive(Clone)]
pub struct GrafanaConfig {
    pub base_url: String,
    pub api_key: Option<String>,
}

impl Default for GrafanaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:3000".to_string(),
            api_key: None,
        }
    }
}

/// Prometheus query routed through Grafana's datasource proxy
#[derive(Clone)]
pub struct GrafanaPrometheusQuery {
    /// UID of the Grafana datasource to query
    pub datasource_uid: String,
    /// PromQL expression to execute
    pub query: String,
    /// Metric label to use as the node identifier
    pub id_label: String,
    /// Health classification thresholds for metric values
    pub thresholds: HealthThresholds,
}

impl Default for GrafanaPrometheusQuery {
    fn default() -> Self {
        Self {
            datasource_uid: "prometheus".to_string(),
            query: "up".to_string(),
            id_label: "instance".to_string(),
            thresholds: HealthThresholds::default(),
        }
    }
}

/// Loki query routed through Grafana's datasource proxy
#[derive(Clone)]
pub struct GrafanaLokiQuery {
    /// UID of the Grafana datasource to query
    pub datasource_uid: String,
    /// LogQL expression to execute
    pub query: String,
    /// Stream label to use as the node identifier
    pub id_label: String,
    /// Time window for the log query (in seconds)
    pub range_seconds: u64,
    /// Pattern matching config for classifying log health
    pub log_analysis: LogAnalysisConfig,
}

impl Default for GrafanaLokiQuery {
    fn default() -> Self {
        Self {
            datasource_uid: "loki".to_string(),
            query: r#"{service="crawler"}"#.to_string(),
            id_label: "service".to_string(),
            range_seconds: 300,
            log_analysis: LogAnalysisConfig::default(),
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

/// HTTP client for querying Grafana's datasource proxy API
pub struct GrafanaClient {
    pub config: GrafanaConfig,
    client: reqwest::Client,
}

impl GrafanaClient {
    pub fn new(config: GrafanaConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(5))
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    pub async fn fetch_nodes(&self, query: &GrafanaPrometheusQuery) -> Result<Vec<NodeStatus>, DataError> {
        let url = format!("{}/api/ds/query", self.config.base_url);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| DataError::ParseError(format!("System clock error: {e}")))?
            .as_millis() as i64;

        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": {
                    "uid": query.datasource_uid
                },
                "expr": query.query,
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

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(DataError::AuthError(format!(
                "Grafana authentication failed (HTTP {})",
                response.status().as_u16()
            )));
        }

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "Grafana query failed: HTTP {}",
                response.status()
            )));
        }

        let body: GrafanaQueryResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        let mut nodes = Vec::new();

        let result = body.results.get("A").ok_or_else(|| {
            DataError::ParseError("Grafana response missing expected result 'A'".to_string())
        })?;

        for frame in &result.frames {
            for (i, field) in frame.schema.fields.iter().enumerate() {
                if field.name == "Value" {
                    if let Some(labels) = &field.labels {
                        let id = labels
                            .get(&query.id_label)
                            .cloned()
                            .unwrap_or_else(|| {
                                warn!("Grafana: missing '{}' label, using fallback ID", query.id_label);
                                format!("node-{}", i)
                            });

                        let lat = labels
                            .get("lat")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or_else(|| {
                                warn!("Grafana node {id}: missing or invalid 'lat' label, defaulting to 0.0");
                                0.0
                            });
                        let lon = labels
                            .get("lon")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or_else(|| {
                                warn!("Grafana node {id}: missing or invalid 'lon' label, defaulting to 0.0");
                                0.0
                            });

                        let value = frame
                            .data
                            .values
                            .get(i)
                            .and_then(|vals| vals.last())
                            .and_then(|v| v.as_f64());

                        let Some(value) = value else {
                            warn!("Grafana node {id}: no valid metric value, skipping");
                            continue;
                        };

                        let health = query.thresholds.classify(value);

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

        Ok(nodes)
    }

    pub async fn fetch_logs(&self, query: &GrafanaLokiQuery) -> Result<Vec<NodeStatus>, DataError> {
        let url = format!("{}/api/ds/query", self.config.base_url);

        let from = format!("now-{}s", query.range_seconds);

        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": {
                    "uid": query.datasource_uid
                },
                "expr": query.query,
                "queryType": "range",
                "maxLines": 1000
            }],
            "from": from,
            "to": "now"
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(DataError::AuthError(format!(
                "Grafana authentication failed (HTTP {})",
                response.status().as_u16()
            )));
        }

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "Grafana/Loki query failed: HTTP {}",
                response.status()
            )));
        }

        let body: GrafanaQueryResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        let result = body.results.get("A").ok_or_else(|| {
            DataError::ParseError("Grafana response missing expected result 'A'".to_string())
        })?;

        let mut nodes = Vec::new();

        for frame in &result.frames {
            let labels_idx = frame.schema.fields.iter().position(|f| f.name == "labels");
            let line_idx = frame.schema.fields.iter().position(|f| f.name == "Line");

            let (Some(labels_idx), Some(line_idx)) = (labels_idx, line_idx) else {
                warn!("Grafana/Loki: frame missing 'labels' or 'Line' field, skipping");
                continue;
            };

            let labels_values = frame.data.values.get(labels_idx);
            let line_values = frame.data.values.get(line_idx);

            let (Some(labels_values), Some(line_values)) = (labels_values, line_values) else {
                warn!("Grafana/Loki: frame has schema fields but data arrays are missing, skipping");
                continue;
            };

            // Group log lines by their labels (each unique label set = one node)
            let mut grouped: HashMap<String, Vec<(String, String)>> = HashMap::new();

            for (i, label_val) in labels_values.iter().enumerate() {
                let label_str = label_val.as_str().unwrap_or("{}");
                let line = line_values.get(i).and_then(|v| v.as_str()).unwrap_or("");
                grouped
                    .entry(label_str.to_string())
                    .or_default()
                    .push((i.to_string(), line.to_string()));
            }

            for (label_json, logs) in &grouped {
                let labels: HashMap<String, String> = match serde_json::from_str(label_json) {
                    Ok(l) => l,
                    Err(e) => {
                        warn!(
                            "Grafana/Loki: failed to parse label JSON, skipping entry: {e}"
                        );
                        continue;
                    }
                };

                let id = labels
                    .get(&query.id_label)
                    .cloned()
                    .unwrap_or_else(|| {
                        warn!("Grafana/Loki: missing '{}' label, using fallback", query.id_label);
                        "unknown".to_string()
                    });

                let lat = labels
                    .get("lat")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| {
                        warn!("Grafana/Loki node {id}: missing or invalid 'lat' label, defaulting to 0.0");
                        0.0
                    });
                let lon = labels
                    .get("lon")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| {
                        warn!("Grafana/Loki node {id}: missing or invalid 'lon' label, defaulting to 0.0");
                        0.0
                    });

                let (health, metrics) = super::analyze_logs(logs, &query.log_analysis);

                nodes.push(NodeStatus {
                    id,
                    lat,
                    lon,
                    health,
                    metrics,
                    last_updated: Instant::now(),
                });
            }
        }

        Ok(nodes)
    }

    /// Fetches raw log entries from Loki via Grafana (for display in classifier panel)
    pub async fn fetch_raw_logs(&self, query: &GrafanaLokiQuery) -> Result<Vec<RawLogEntry>, DataError> {
        let url = format!("{}/api/ds/query", self.config.base_url);

        let from = format!("now-{}s", query.range_seconds);

        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": {
                    "uid": query.datasource_uid
                },
                "expr": query.query,
                "queryType": "range",
                "maxLines": 100
            }],
            "from": from,
            "to": "now"
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err(DataError::AuthError(format!(
                "Grafana authentication failed (HTTP {})",
                response.status().as_u16()
            )));
        }

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "Grafana/Loki query failed: HTTP {}",
                response.status()
            )));
        }

        let body: GrafanaQueryResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        let result = body.results.get("A").ok_or_else(|| {
            DataError::ParseError("Grafana response missing expected result 'A'".to_string())
        })?;

        let mut entries = Vec::new();

        for frame in &result.frames {
            let labels_idx = frame.schema.fields.iter().position(|f| f.name == "labels");
            let line_idx = frame.schema.fields.iter().position(|f| f.name == "Line");

            let (Some(labels_idx), Some(line_idx)) = (labels_idx, line_idx) else {
                continue;
            };

            let labels_values = frame.data.values.get(labels_idx);
            let line_values = frame.data.values.get(line_idx);

            let (Some(labels_values), Some(line_values)) = (labels_values, line_values) else {
                continue;
            };

            for (i, label_val) in labels_values.iter().enumerate() {
                let label_str = label_val.as_str().unwrap_or("{}");
                let message = line_values.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();

                let labels: HashMap<String, String> = serde_json::from_str(label_str).unwrap_or_default();
                let source = labels
                    .get(&query.id_label)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());

                entries.push(RawLogEntry {
                    timestamp: Instant::now(),
                    source,
                    message,
                    labels,
                });
            }
        }

        Ok(entries)
    }

    /// Fetches a single metric count from Loki via Grafana (for frontier stats)
    pub async fn fetch_loki_count(&self, query: &str, range_seconds: u64) -> Result<u64, DataError> {
        let url = format!("{}/api/ds/query", self.config.base_url);

        let from = format!("now-{}s", range_seconds);

        let body = serde_json::json!({
            "queries": [{
                "refId": "A",
                "datasource": {
                    "uid": "loki"
                },
                "expr": query,
                "queryType": "instant"
            }],
            "from": from,
            "to": "now"
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(ref api_key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .await
            .map_err(|e| DataError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DataError::NetworkError(format!(
                "Grafana/Loki count query failed: HTTP {}",
                response.status()
            )));
        }

        let body: GrafanaQueryResponse = response
            .json()
            .await
            .map_err(|e| DataError::ParseError(e.to_string()))?;

        let result = body.results.get("A").ok_or_else(|| {
            DataError::ParseError("Grafana response missing expected result 'A'".to_string())
        })?;

        // Extract the count value from the first frame
        for frame in &result.frames {
            if let Some(values) = frame.data.values.first() {
                if let Some(val) = values.first() {
                    if let Some(n) = val.as_f64() {
                        return Ok(n as u64);
                    }
                    if let Some(n) = val.as_u64() {
                        return Ok(n);
                    }
                }
            }
        }

        Ok(0)
    }

    /// Fetches all frontier stats from Loki via Grafana
    pub async fn fetch_frontier_stats(&self, range_seconds: u64) -> FrontierStatsResult {
        let range = format!("[{}s]", range_seconds);

        let submit_events = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "URL submitted to frontier" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        let new_urls_queued = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "URL submitted to frontier" | json | queued="true" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        let fetch_success = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "URL fetched successfully" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        let fetch_failures = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "URL fetch failed" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        let robots_blocked = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "robots_blocked" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        let dead_urls = self
            .fetch_loki_count(
                &format!(r#"sum(count_over_time({{service="crawler"}} |= "URL marked dead" {}))"#, range),
                range_seconds,
            )
            .await
            .unwrap_or(0);

        // Queue depth from frontier_stats logs (last value in 2h window)
        let pending = self
            .fetch_loki_count(
                r#"max(last_over_time({service="crawler"} |= "frontier_stats" | json | unwrap total_pending [2h]))"#,
                7200,
            )
            .await
            .unwrap_or(0);

        let fetching = self
            .fetch_loki_count(
                r#"max(last_over_time({service="crawler"} |= "frontier_stats" | json | unwrap total_fetching [2h]))"#,
                7200,
            )
            .await
            .unwrap_or(0);

        FrontierStatsResult {
            submit_events,
            new_urls_queued,
            fetch_success,
            fetch_failures,
            robots_blocked,
            dead_urls,
            pending,
            fetching,
        }
    }
}

/// Result from fetching frontier stats
#[derive(Clone, Debug, Default)]
pub struct FrontierStatsResult {
    pub submit_events: u64,
    pub new_urls_queued: u64,
    pub fetch_success: u64,
    pub fetch_failures: u64,
    pub robots_blocked: u64,
    pub dead_urls: u64,
    pub pending: u64,
    pub fetching: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grafana_config_defaults() {
        let config = GrafanaConfig::default();
        assert_eq!(config.base_url, "http://localhost:3000");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn prometheus_query_defaults() {
        let q = GrafanaPrometheusQuery::default();
        assert_eq!(q.datasource_uid, "prometheus");
        assert_eq!(q.query, "up");
        assert_eq!(q.id_label, "instance");
    }

    #[test]
    fn loki_query_defaults() {
        let q = GrafanaLokiQuery::default();
        assert_eq!(q.datasource_uid, "loki");
        assert_eq!(q.id_label, "service");
        assert_eq!(q.range_seconds, 300);
        assert!(!q.log_analysis.critical_patterns.is_empty());
    }

    #[test]
    fn parse_grafana_loki_response() {
        let json = serde_json::json!({
            "results": {
                "A": {
                    "frames": [
                        {
                            "schema": {
                                "fields": [
                                    {"name": "labels", "type": "string"},
                                    {"name": "Time", "type": "time"},
                                    {"name": "Line", "type": "string"}
                                ]
                            },
                            "data": {
                                "values": [
                                    ["{\"service\":\"crawler\"}"],
                                    [1709000000000_i64],
                                    ["error: connection refused"]
                                ]
                            }
                        }
                    ]
                }
            }
        });

        let response: GrafanaQueryResponse = serde_json::from_value(json).unwrap();
        let result = response.results.get("A").unwrap();
        assert_eq!(result.frames.len(), 1);

        let frame = &result.frames[0];
        let line_idx = frame.schema.fields.iter().position(|f| f.name == "Line");
        assert!(line_idx.is_some());
    }
}
