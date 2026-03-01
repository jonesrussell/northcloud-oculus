# Grafana Bridge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Route all VR observability queries (Prometheus metrics + Loki logs) through Grafana's datasource proxy API using a single service account token.

**Architecture:** Consolidate the three independent HTTP clients (PrometheusClient, GrafanaClient, LokiClient) into a single GrafanaClient that queries both Prometheus and Loki via `POST /api/ds/query`. Config loaded from env vars.

**Tech Stack:** Rust, reqwest, serde_json, Bevy 0.18 (resources/systems)

**Design doc:** `docs/plans/2026-02-28-grafana-bridge-design.md`

---

### Task 1: Extract log analysis into a standalone function + tests

The `analyze_logs` method currently lives on `LokiClient`. We need it reusable for both `LokiClient` and the new Grafana Loki path. Extract it first with tests so we don't break it during the refactor.

**Files:**
- Modify: `src/data/node_status.rs`
- Modify: `src/data/loki.rs`

**Step 1: Write failing tests for log analysis**

Add to `src/data/node_status.rs`:

```rust
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
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib data::node_status::tests -- --nocapture`
Expected: compile error — `analyze_logs` and `LogAnalysisConfig` don't exist yet.

**Step 3: Add `LogAnalysisConfig` and `analyze_logs` to `node_status.rs`**

Add to `src/data/node_status.rs` (before the tests module):

```rust
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
```

**Step 4: Update `LokiClient` to use the extracted function**

In `src/data/loki.rs`, replace the `analyze_logs` method body:

```rust
fn analyze_logs(&self, logs: &[(String, String)]) -> (NodeHealth, HashMap<String, f64>) {
    let config = LogAnalysisConfig {
        critical_patterns: self.config.critical_patterns.clone(),
        warning_patterns: self.config.warning_patterns.clone(),
    };
    super::analyze_logs(logs, &config)
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test --lib data::node_status::tests -- --nocapture`
Expected: all 5 tests pass.

**Step 6: Verify full build**

Run: `cargo check`
Expected: compiles with no errors.

**Step 7: Commit**

```bash
git add src/data/node_status.rs src/data/loki.rs
git commit -m "refactor: extract log analysis into standalone function with tests"
```

---

### Task 2: Restructure GrafanaConfig for connection + query separation

The current `GrafanaConfig` mixes connection details (base_url, api_key) with Prometheus query details (datasource_uid, query, id_label, thresholds). Split these so one connection config can drive both Prometheus and Loki queries.

**Files:**
- Modify: `src/data/grafana.rs`
- Modify: `src/data/mod.rs`

**Step 1: Write failing tests for new config types**

Add to bottom of `src/data/grafana.rs`:

```rust
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
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib data::grafana::tests -- --nocapture`
Expected: compile error — `GrafanaPrometheusQuery` and `GrafanaLokiQuery` don't exist yet.

**Step 3: Add new config types to `grafana.rs`**

Replace the existing `GrafanaConfig` struct and its `Default` impl with:

```rust
/// Grafana connection configuration (shared by all queries)
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
    pub datasource_uid: String,
    pub query: String,
    pub id_label: String,
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
    pub datasource_uid: String,
    pub query: String,
    pub id_label: String,
    pub range_seconds: u64,
    pub log_analysis: LogAnalysisConfig,
}

impl Default for GrafanaLokiQuery {
    fn default() -> Self {
        Self {
            datasource_uid: "loki".to_string(),
            query: r#"{job="varlogs"}"#.to_string(),
            id_label: "service".to_string(),
            range_seconds: 300,
            log_analysis: LogAnalysisConfig::default(),
        }
    }
}
```

Add `use super::LogAnalysisConfig;` to the imports at the top of `grafana.rs`.

**Step 4: Update `GrafanaClient` to use new config**

Update the `GrafanaClient` struct and constructor:

```rust
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
}
```

**Step 5: Update `fetch_nodes` to accept query config as parameter**

Change the `DataSource` impl to a direct method that takes a `GrafanaPrometheusQuery`:

```rust
impl GrafanaClient {
    // ... new() above ...

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
}
```

Remove the `#[async_trait::async_trait] impl DataSource for GrafanaClient` block (replaced by the direct method above).

**Step 6: Update `DataIngestionConfig` in `mod.rs`**

Replace the config struct and builder methods:

```rust
#[derive(Resource)]
pub struct DataIngestionConfig {
    pub poll_interval_secs: f32,
    pub grafana: Option<GrafanaConfig>,
    pub prometheus_query: Option<GrafanaPrometheusQuery>,
    pub loki_query: Option<GrafanaLokiQuery>,
}

impl Default for DataIngestionConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30.0,
            grafana: None,
            prometheus_query: None,
            loki_query: None,
        }
    }
}

impl DataIngestionConfig {
    pub fn with_grafana(mut self, config: GrafanaConfig) -> Self {
        self.grafana = Some(config);
        self
    }

    pub fn with_prometheus_query(mut self, query: GrafanaPrometheusQuery) -> Self {
        self.prometheus_query = Some(query);
        self
    }

    pub fn with_loki_query(mut self, query: GrafanaLokiQuery) -> Self {
        self.loki_query = Some(query);
        self
    }
}
```

**Step 7: Update `poll_data_sources` in `mod.rs`**

Replace the system to use only `GrafanaClient`:

```rust
pub fn poll_data_sources(
    time: Res<Time>,
    config: Res<DataIngestionConfig>,
    mut state: ResMut<DataIngestionState>,
) {
    if state.pending_task.is_some() {
        return;
    }

    let should_poll = match state.last_poll {
        Some(last) => time.elapsed_secs() - last >= config.poll_interval_secs,
        None => true,
    };

    if !should_poll {
        return;
    }

    state.last_poll = Some(time.elapsed_secs());

    let grafana_config = config.grafana.clone();
    let prometheus_query = config.prometheus_query.clone();
    let loki_query = config.loki_query.clone();

    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        let mut all_nodes = Vec::new();

        let Some(grafana_config) = grafana_config else {
            return all_nodes;
        };

        let client = GrafanaClient::new(grafana_config);

        if let Some(ref prom_query) = prometheus_query {
            match client.fetch_nodes(prom_query).await {
                Ok(nodes) => all_nodes.extend(nodes),
                Err(e) => warn!("Grafana/Prometheus fetch failed: {e}"),
            }
        }

        if let Some(ref loki_query) = loki_query {
            match client.fetch_logs(loki_query).await {
                Ok(nodes) => all_nodes.extend(nodes),
                Err(e) => warn!("Grafana/Loki fetch failed: {e}"),
            }
        }

        all_nodes
    });

    state.pending_task = Some(task);
}
```

**Step 8: Run tests and check build**

Run: `cargo test --lib data -- --nocapture`
Expected: all tests pass.

Run: `cargo check`
Expected: compiles (will have warnings about unused `PrometheusClient`, `LokiClient`, `DataSource` trait — that's fine for now).

**Step 9: Commit**

```bash
git add src/data/grafana.rs src/data/mod.rs
git commit -m "refactor: restructure GrafanaConfig for unified connection + query configs"
```

---

### Task 3: Add Loki query method to GrafanaClient

Add `fetch_logs()` that sends a Loki LogQL query through Grafana's `/api/ds/query` and parses the response into `NodeStatus` entries.

**Files:**
- Modify: `src/data/grafana.rs`

**Step 1: Write test for Loki response parsing**

Add to the `tests` module in `src/data/grafana.rs`:

```rust
#[test]
fn parse_grafana_loki_response() {
    // Grafana returns Loki data as frames with labels JSON + lines
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
    // Verify we can find the Line field
    let line_idx = frame.schema.fields.iter().position(|f| f.name == "Line");
    assert!(line_idx.is_some());
}
```

**Step 2: Run test to verify it passes (parsing test uses existing types)**

Run: `cargo test --lib data::grafana::tests::parse_grafana_loki_response -- --nocapture`
Expected: PASS (the existing `GrafanaQueryResponse` struct can deserialize this).

Note: If the `type` field in the schema causes a deserialize error (it's not in the current struct), update `GrafanaField` to add `#[serde(default)] pub r#type: Option<String>` or just use `#[serde(flatten)] pub extra: HashMap<String, serde_json::Value>`. The simpler fix is to not require the type field — serde ignores unknown fields by default unless `#[serde(deny_unknown_fields)]` is set, which it isn't.

**Step 3: Add `fetch_logs` method to `GrafanaClient`**

Add this method to the `impl GrafanaClient` block in `src/data/grafana.rs`:

```rust
pub async fn fetch_logs(&self, query: &GrafanaLokiQuery) -> Result<Vec<NodeStatus>, DataError> {
    let url = format!("{}/api/ds/query", self.config.base_url);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| DataError::ParseError(format!("System clock error: {e}")))?
        .as_millis() as i64;

    let from = now - (query.range_seconds as i64 * 1000);

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
        "from": from.to_string(),
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
        // Find field indices
        let labels_idx = frame.schema.fields.iter().position(|f| f.name == "labels");
        let line_idx = frame.schema.fields.iter().position(|f| f.name == "Line");

        let (Some(labels_idx), Some(line_idx)) = (labels_idx, line_idx) else {
            warn!("Grafana/Loki: frame missing 'labels' or 'Line' field, skipping");
            continue;
        };

        let labels_values = frame.data.values.get(labels_idx);
        let line_values = frame.data.values.get(line_idx);

        let (Some(labels_values), Some(line_values)) = (labels_values, line_values) else {
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
            // Parse the labels JSON to extract the node ID
            let labels: HashMap<String, String> =
                serde_json::from_str(label_json).unwrap_or_default();

            let id = labels
                .get(&query.id_label)
                .cloned()
                .unwrap_or_else(|| {
                    warn!("Grafana/Loki: missing '{}' label, using fallback", query.id_label);
                    "unknown".to_string()
                });

            let lat = labels.get("lat").and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let lon = labels.get("lon").and_then(|v| v.parse().ok()).unwrap_or(0.0);

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
```

**Step 4: Run tests and check build**

Run: `cargo test --lib data -- --nocapture`
Expected: all tests pass.

Run: `cargo check`
Expected: compiles.

**Step 5: Commit**

```bash
git add src/data/grafana.rs
git commit -m "feat: add Loki log query via Grafana datasource proxy"
```

---

### Task 4: Add env var config loading

Add a factory function to build `DataIngestionConfig` from environment variables so the Grafana URL and token aren't hardcoded.

**Files:**
- Modify: `src/data/mod.rs`
- Modify: `.env.example`

**Step 1: Write failing test for env-based config**

Add to `src/data/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_with_grafana_url() {
        // Set env vars for this test
        std::env::set_var("GRAFANA_URL", "https://northcloud.one/grafana");
        std::env::set_var("GRAFANA_TOKEN", "test-token-123");

        let config = DataIngestionConfig::from_env();

        let grafana = config.grafana.expect("grafana config should be set");
        assert_eq!(grafana.base_url, "https://northcloud.one/grafana");
        assert_eq!(grafana.api_key.unwrap(), "test-token-123");

        // Clean up
        std::env::remove_var("GRAFANA_URL");
        std::env::remove_var("GRAFANA_TOKEN");
    }

    #[test]
    fn config_from_env_without_grafana_url() {
        std::env::remove_var("GRAFANA_URL");
        std::env::remove_var("GRAFANA_TOKEN");

        let config = DataIngestionConfig::from_env();
        assert!(config.grafana.is_none());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --lib data::tests -- --nocapture`
Expected: compile error — `from_env` doesn't exist.

**Step 3: Implement `from_env`**

Add to `impl DataIngestionConfig` in `src/data/mod.rs`:

```rust
/// Build config from environment variables.
///
/// Reads:
/// - `GRAFANA_URL` — Grafana base URL (required to enable data ingestion)
/// - `GRAFANA_TOKEN` — Grafana service account token (optional in dev, required in prod)
/// - `POLL_INTERVAL_SECS` — polling interval (optional, default 30)
pub fn from_env() -> Self {
    let grafana_url = std::env::var("GRAFANA_URL").ok();
    let grafana_token = std::env::var("GRAFANA_TOKEN").ok();
    let poll_interval: f32 = std::env::var("POLL_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30.0);

    let grafana = grafana_url.map(|url| GrafanaConfig {
        base_url: url,
        api_key: grafana_token,
    });

    let prometheus_query = if grafana.is_some() {
        Some(GrafanaPrometheusQuery::default())
    } else {
        None
    };

    let loki_query = if grafana.is_some() {
        Some(GrafanaLokiQuery::default())
    } else {
        None
    };

    Self {
        poll_interval_secs: poll_interval,
        grafana,
        prometheus_query,
        loki_query,
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib data::tests -- --nocapture --test-threads=1`
Expected: both tests pass. Note: `--test-threads=1` because tests mutate env vars.

**Step 5: Update `.env.example`**

Replace contents of `.env.example`:

```
# Grafana connection (required for data ingestion)
GRAFANA_URL=https://northcloud.one/grafana
GRAFANA_TOKEN=

# Polling interval in seconds (optional, default: 30)
POLL_INTERVAL_SECS=30
```

**Step 6: Commit**

```bash
git add src/data/mod.rs .env.example
git commit -m "feat: add env-based config loading for Grafana bridge"
```

---

### Task 5: Wire up config in main.rs

Use `DataIngestionConfig::from_env()` to load config at startup.

**Files:**
- Modify: `src/main.rs`

**Step 1: Update main.rs to load config from env**

In `src/main.rs`, after `App::new()` and before `.run()`, replace the `.add_plugins(DataIngestionPlugin)` line. Insert the config resource before the plugin:

```rust
.insert_resource(northcloud_oculus::data::DataIngestionConfig::from_env())
.add_plugins(DataIngestionPlugin)
```

Since `DataIngestionConfig` is now inserted before the plugin runs, update `DataIngestionPlugin::build` in `src/data/mod.rs` to NOT call `init_resource::<DataIngestionConfig>()` (which would overwrite the env-loaded config with defaults). Change it to only init if not already present:

In `src/data/mod.rs`, update the plugin build:

```rust
impl Plugin for DataIngestionPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<DataIngestionConfig>() {
            app.init_resource::<DataIngestionConfig>();
        }
        app.init_resource::<NodeStatusBuffer>()
            .init_resource::<DataIngestionState>()
            .add_systems(Update, (poll_data_sources, apply_node_status_updates).chain());
    }
}
```

**Step 2: Verify build**

Run: `cargo check`
Expected: compiles with no errors. May have warnings about unused `PrometheusClient`, `LokiClient` — addressed in Task 6.

**Step 3: Commit**

```bash
git add src/main.rs src/data/mod.rs
git commit -m "feat: wire env-based Grafana config into app startup"
```

---

### Task 6: Clean up unused direct clients

The `PrometheusClient` and `LokiClient` are no longer used in the polling loop. The `DataSource` trait is also unused now. Remove them from the polling path and mark the old modules as available but not actively used.

**Files:**
- Modify: `src/data/mod.rs`

**Step 1: Remove old imports and re-exports from `mod.rs`**

In `src/data/mod.rs`:
- Remove `use crate::node_marker::NodeMarker;` if it's no longer needed (check `apply_node_status_updates` — it still uses `NodeMarker`, so keep it).
- Remove `pub use prometheus::*;` and `pub use loki::*;` from the re-exports. The modules still exist for reference, but aren't part of the public API.
- Keep `mod prometheus;` and `mod loki;` declarations so the code compiles but add `#[allow(dead_code)]` or just remove the modules entirely.

Decision: **Keep the modules** but don't re-export. They serve as reference implementations and could be useful if direct access is needed later.

Update the top of `src/data/mod.rs`:

```rust
mod node_status;
mod prometheus;
mod grafana;
mod loki;

pub use node_status::*;
pub use grafana::*;
// prometheus and loki modules retained but not re-exported
// (all queries now route through GrafanaClient)
```

**Step 2: Run all tests**

Run: `cargo test --lib -- --nocapture --test-threads=1`
Expected: all tests pass.

**Step 3: Run full build**

Run: `cargo check`
Expected: compiles. There may be dead_code warnings for `PrometheusClient`, `LokiClient`, `DataSource` — add `#[allow(dead_code)]` at the module level in `prometheus.rs` and `loki.rs` if desired, or just accept the warnings.

**Step 4: Commit**

```bash
git add src/data/mod.rs
git commit -m "refactor: route all queries through GrafanaClient, retain legacy clients as reference"
```

---

### Task 7: Final verification and cleanup

**Files:**
- All modified files

**Step 1: Run full test suite**

Run: `cargo test --test-threads=1`
Expected: all tests pass.

**Step 2: Run clippy**

Run: `cargo clippy -- -W clippy::all`
Expected: no errors. Fix any warnings.

**Step 3: Verify .env.example is correct**

Run: `cat .env.example`
Expected: shows `GRAFANA_URL`, `GRAFANA_TOKEN`, `POLL_INTERVAL_SECS`.

**Step 4: Final commit if any clippy fixes**

```bash
git add -A
git commit -m "chore: clippy fixes and final cleanup"
```

---

## Manual Steps (north-cloud, not automated)

After implementing the above, complete these one-time steps on the production Grafana instance:

1. Open `https://northcloud.one/grafana/`
2. Go to **Administration > Service Accounts**
3. Create account: name `oculus-vr`, role **Viewer**
4. Generate a token, copy it
5. Set `GRAFANA_TOKEN=<token>` in the VR client's environment
6. Set `GRAFANA_URL=https://northcloud.one/grafana` in the VR client's environment
7. Test: run the VR app and verify data appears in the panel
