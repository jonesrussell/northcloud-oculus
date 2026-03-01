//! Data ingestion via the Grafana datasource proxy API (Prometheus and Loki queries)

mod node_status;
mod prometheus;
mod grafana;
mod loki;

pub use node_status::*;
pub use grafana::*;
// prometheus and loki modules retained but not re-exported
// (all queries now route through GrafanaClient)

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use bevy_egui::egui;
use std::collections::VecDeque;
use std::time::Instant;

use crate::node_marker::NodeMarker;

// --- Shared types (used by both data ingestion and panel rendering) ---

/// A single log entry with a locally-classified severity level
#[derive(Clone, Debug)]
pub struct LogEntry {
    pub fetched_at: Instant,
    pub source: String,
    pub message: String,
    pub level: LogLevel,
}

/// Log severity level, classified by case-insensitive substring matching.
/// Checks for error/fatal/panic first (Error), then warn (Warning), otherwise Info.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn color(&self) -> egui::Color32 {
        match self {
            LogLevel::Info => egui::Color32::from_rgb(180, 180, 180),
            LogLevel::Warning => egui::Color32::from_rgb(255, 200, 50),
            LogLevel::Error => egui::Color32::from_rgb(255, 80, 80),
        }
    }
}

/// Buffer holding recent log entries for display.
/// New entries replace the previous batch each fetch cycle (no cross-cycle accumulation).
#[derive(Resource)]
pub struct LogBuffer {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
    pub last_fetch: Option<Instant>,
    pub fetch_error: Option<String>,
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100,
            last_fetch: None,
            fetch_error: None,
        }
    }
}

impl LogBuffer {
    pub fn push(&mut self, entry: LogEntry) {
        self.entries.push_front(entry);
        while self.entries.len() > self.max_entries {
            self.entries.pop_back();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Frontier statistics from Loki log aggregation
#[derive(Resource, Default)]
pub struct FrontierStats {
    pub data: FrontierStatsResult,
    pub last_updated: Option<Instant>,
    pub fetch_error: Option<String>,
}

// --- Plugin and systems ---

/// Plugin that adds data ingestion functionality
pub struct DataIngestionPlugin;

impl Plugin for DataIngestionPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<DataIngestionConfig>() {
            app.init_resource::<DataIngestionConfig>();
        }
        app.init_resource::<NodeStatusBuffer>()
            .init_resource::<LogBuffer>()
            .init_resource::<FrontierStats>()
            .init_resource::<DataIngestionState>()
            .add_systems(Update, (poll_data_sources, apply_node_status_updates).chain());
    }
}

/// Configuration for data ingestion
#[derive(Resource)]
pub struct DataIngestionConfig {
    /// Poll interval in seconds
    pub poll_interval_secs: f32,
    /// Grafana connection (None = data ingestion disabled)
    pub grafana: Option<GrafanaConfig>,
    /// Prometheus query via Grafana (None = disabled)
    pub prometheus_query: Option<GrafanaPrometheusQuery>,
    /// Loki query via Grafana (None = disabled)
    pub loki_query: Option<GrafanaLokiQuery>,
}

impl Default for DataIngestionConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 10.0,
            grafana: None,
            prometheus_query: None,
            loki_query: None,
        }
    }
}

/// Result from data ingestion task
pub struct DataIngestionResult {
    pub nodes: Vec<NodeStatus>,
    pub logs: Vec<RawLogEntry>,
    pub log_error: Option<String>,
    pub frontier_stats: Option<FrontierStatsResult>,
}

/// Internal state for data polling
#[derive(Resource, Default)]
pub struct DataIngestionState {
    pub last_poll: Option<f32>,
    pub pending_task: Option<Task<DataIngestionResult>>,
}

/// System that polls data sources on an interval
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
        // Create a Tokio runtime for reqwest (which requires Tokio)
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create Tokio runtime for data ingestion: {e}");
                return DataIngestionResult {
                    nodes: Vec::new(),
                    logs: Vec::new(),
                    log_error: Some(format!("Runtime creation failed: {e}")),
                    frontier_stats: None,
                };
            }
        };

        rt.block_on(async {
            let mut result = DataIngestionResult {
                nodes: Vec::new(),
                logs: Vec::new(),
                log_error: None,
                frontier_stats: None,
            };

            let Some(grafana_config) = grafana_config else {
                warn!("No Grafana config — data ingestion disabled");
                return result;
            };

            info!("Polling Grafana at {}", grafana_config.base_url);
            let client = GrafanaClient::new(grafana_config);

            if let Some(ref prom_query) = prometheus_query {
                match client.fetch_nodes(prom_query).await {
                    Ok(nodes) => {
                        info!("Prometheus: fetched {} nodes", nodes.len());
                        result.nodes.extend(nodes);
                    }
                    Err(e) => warn!("Grafana/Prometheus fetch failed: {e}"),
                }
            }

            if let Some(ref loki_query) = loki_query {
                // Fetch analyzed nodes for health status
                match client.fetch_logs(loki_query).await {
                    Ok(nodes) => {
                        info!("Loki logs: fetched {} nodes", nodes.len());
                        result.nodes.extend(nodes);
                    }
                    Err(e) => warn!("Grafana/Loki fetch failed: {e}"),
                }
                // Also fetch raw logs for classifier panel
                match client.fetch_raw_logs(loki_query).await {
                    Ok(logs) => {
                        info!("Loki raw logs: fetched {} entries", logs.len());
                        result.logs = logs;
                    }
                    Err(e) => {
                        warn!("Grafana/Loki raw log fetch failed: {e}");
                        result.log_error = Some(e.to_string());
                    }
                }

                // Fetch frontier stats (counters use 24h window; queue depth uses 2h snapshot)
                info!("Fetching frontier stats...");
                let frontier = client
                    .fetch_frontier_stats(86400, &loki_query.datasource_uid, "crawler")
                    .await;
                info!(
                    "Frontier stats: submitted={}, fetched={}, pending={}, errors={}",
                    frontier.submit_events, frontier.fetch_success, frontier.pending,
                    frontier.errors.len()
                );
                result.frontier_stats = Some(frontier);
            }

            result
        })
    });

    state.pending_task = Some(task);
}

/// System that applies received node status updates to the buffer and markers
pub fn apply_node_status_updates(
    mut state: ResMut<DataIngestionState>,
    mut buffer: ResMut<NodeStatusBuffer>,
    mut log_buffer: ResMut<LogBuffer>,
    mut frontier_stats: ResMut<FrontierStats>,
    mut markers: Query<&mut NodeMarker>,
) {
    let Some(ref mut task) = state.pending_task else {
        return;
    };

    if !task.is_finished() {
        return;
    }

    let mut task = state.pending_task.take().unwrap();
    let result = block_on(poll_once(&mut task)).unwrap_or_else(|| {
        warn!("Data ingestion task returned no result despite being finished");
        DataIngestionResult {
            nodes: Vec::new(),
            logs: Vec::new(),
            log_error: Some("Task failed".to_string()),
            frontier_stats: None,
        }
    });

    // Update node status buffer and markers
    for node in result.nodes {
        if let Some(mut marker) = markers.iter_mut().find(|m| m.id == node.id) {
            marker.health = node.health;
        }
        buffer.update(node);
    }

    // Update log buffer for classifier panel — clear old entries then add new batch
    log_buffer.last_fetch = Some(Instant::now());
    log_buffer.fetch_error = result.log_error;
    log_buffer.clear();
    for raw_log in result.logs {
        let level = classify_log_level(&raw_log.message);
        log_buffer.push(LogEntry {
            fetched_at: raw_log.fetched_at,
            source: raw_log.source,
            message: raw_log.message,
            level,
        });
    }

    // Update frontier stats
    if let Some(stats_result) = result.frontier_stats {
        frontier_stats.fetch_error = if stats_result.errors.is_empty() {
            None
        } else {
            Some(stats_result.errors.join("; "))
        };
        frontier_stats.data = stats_result;
        frontier_stats.last_updated = Some(Instant::now());
    }
}

/// Classify log level based on case-insensitive substring matching.
/// Uses the same patterns as `LogAnalysisConfig::default()`.
fn classify_log_level(message: &str) -> LogLevel {
    let lower = message.to_lowercase();
    if lower.contains("error") || lower.contains("fatal") || lower.contains("panic") {
        LogLevel::Error
    } else if lower.contains("warn") {
        LogLevel::Warning
    } else {
        LogLevel::Info
    }
}

/// Builder methods for DataIngestionConfig
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

    /// Build config from environment variables.
    ///
    /// Reads:
    /// - `GRAFANA_URL` — Grafana base URL (required to enable data ingestion)
    /// - `GRAFANA_TOKEN` — Grafana service account token (optional; required if Grafana has auth enabled)
    /// - `POLL_INTERVAL_SECS` — polling interval (optional, default 10)
    pub fn from_env() -> Self {
        Self::from_env_vars(
            std::env::var("GRAFANA_URL").ok(),
            std::env::var("GRAFANA_TOKEN").ok(),
            std::env::var("POLL_INTERVAL_SECS").ok(),
        )
    }

    fn from_env_vars(
        grafana_url: Option<String>,
        grafana_token: Option<String>,
        poll_interval_str: Option<String>,
    ) -> Self {
        let poll_interval: f32 = match poll_interval_str {
            Some(val) => match val.parse::<f32>() {
                Ok(secs) if secs > 0.0 && secs.is_finite() => secs,
                Ok(secs) => {
                    warn!(
                        "POLL_INTERVAL_SECS={secs} is invalid (must be positive and finite), defaulting to 10"
                    );
                    10.0
                }
                Err(e) => {
                    warn!("POLL_INTERVAL_SECS could not be parsed: {e}, defaulting to 10");
                    10.0
                }
            },
            None => 10.0,
        };

        let grafana = grafana_url.map(|url| {
            info!(
                "Grafana data ingestion enabled: url={}, token={}",
                url,
                if grafana_token.is_some() { "set" } else { "not set" }
            );
            GrafanaConfig {
                base_url: url,
                api_key: grafana_token,
            }
        });

        if grafana.is_none() {
            warn!("GRAFANA_URL not set — data ingestion is disabled");
        }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_with_grafana_url() {
        let config = DataIngestionConfig::from_env_vars(
            Some("https://northcloud.one/grafana".to_string()),
            Some("test-token-123".to_string()),
            None,
        );

        let grafana = config.grafana.expect("grafana config should be set");
        assert_eq!(grafana.base_url, "https://northcloud.one/grafana");
        assert_eq!(grafana.api_key.unwrap(), "test-token-123");
        assert!(config.prometheus_query.is_some());
        assert!(config.loki_query.is_some());
    }

    #[test]
    fn config_without_grafana_url() {
        let config = DataIngestionConfig::from_env_vars(None, None, None);
        assert!(config.grafana.is_none());
        assert!(config.prometheus_query.is_none());
        assert!(config.loki_query.is_none());
    }

    #[test]
    fn config_poll_interval_parsed() {
        let config = DataIngestionConfig::from_env_vars(
            None,
            None,
            Some("15".to_string()),
        );
        assert_eq!(config.poll_interval_secs, 15.0);
    }

    #[test]
    fn config_poll_interval_invalid_falls_back() {
        let config = DataIngestionConfig::from_env_vars(
            None,
            None,
            Some("not-a-number".to_string()),
        );
        assert_eq!(config.poll_interval_secs, 10.0);
    }

    #[test]
    fn config_poll_interval_negative_falls_back() {
        let config = DataIngestionConfig::from_env_vars(
            None,
            None,
            Some("-5".to_string()),
        );
        assert_eq!(config.poll_interval_secs, 10.0);
    }
}
