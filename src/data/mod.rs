//! Data ingestion from Prometheus, Grafana, and Loki

mod node_status;
mod prometheus;
mod grafana;
mod loki;

pub use node_status::*;
pub use prometheus::*;
pub use grafana::*;
pub use loki::*;

use bevy::prelude::*;
use bevy::tasks::{block_on, AsyncComputeTaskPool, Task};

use crate::node_marker::NodeMarker;

/// Plugin that adds data ingestion functionality
pub struct DataIngestionPlugin;

impl Plugin for DataIngestionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NodeStatusBuffer>()
            .init_resource::<DataIngestionConfig>()
            .init_resource::<DataIngestionState>()
            .add_systems(Update, (poll_data_sources, apply_node_status_updates));
    }
}

/// Configuration for data ingestion
#[derive(Resource)]
pub struct DataIngestionConfig {
    /// Poll interval in seconds
    pub poll_interval_secs: f32,
    /// Prometheus configuration (None = disabled)
    pub prometheus: Option<PrometheusConfig>,
    /// Grafana configuration (None = disabled)
    pub grafana: Option<GrafanaConfig>,
    /// Loki configuration (None = disabled)
    pub loki: Option<LokiConfig>,
}

impl Default for DataIngestionConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30.0,
            prometheus: None,
            grafana: None,
            loki: None,
        }
    }
}

/// Internal state for data polling
#[derive(Resource, Default)]
pub struct DataIngestionState {
    pub last_poll: Option<f32>,
    pub pending_task: Option<Task<Vec<NodeStatus>>>,
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

    let prometheus_config = config.prometheus.clone();
    let grafana_config = config.grafana.clone();
    let loki_config = config.loki.clone();

    let task_pool = AsyncComputeTaskPool::get();
    let task = task_pool.spawn(async move {
        let mut all_nodes = Vec::new();

        if let Some(config) = prometheus_config {
            let client = PrometheusClient::new(config);
            if let Ok(nodes) = client.fetch_nodes().await {
                all_nodes.extend(nodes);
            }
        }

        if let Some(config) = grafana_config {
            let client = GrafanaClient::new(config);
            if let Ok(nodes) = client.fetch_nodes().await {
                all_nodes.extend(nodes);
            }
        }

        if let Some(config) = loki_config {
            let client = LokiClient::new(config);
            if let Ok(nodes) = client.fetch_nodes().await {
                all_nodes.extend(nodes);
            }
        }

        all_nodes
    });

    state.pending_task = Some(task);
}

/// System that applies received node status updates to the buffer and markers
pub fn apply_node_status_updates(
    mut state: ResMut<DataIngestionState>,
    mut buffer: ResMut<NodeStatusBuffer>,
    mut markers: Query<&mut NodeMarker>,
) {
    let Some(ref mut task) = state.pending_task else {
        return;
    };

    if !task.is_finished() {
        return;
    }

    let task = state.pending_task.take().unwrap();
    let nodes = block_on(async { task.cancel().await.unwrap_or_default() });

    for node in nodes {
        if let Some(mut marker) = markers.iter_mut().find(|m| m.id == node.id) {
            marker.health = node.health;
        }
        buffer.update(node);
    }
}

/// Helper to create a DataIngestionConfig with mock data for testing
impl DataIngestionConfig {
    pub fn with_prometheus(mut self, config: PrometheusConfig) -> Self {
        self.prometheus = Some(config);
        self
    }

    pub fn with_grafana(mut self, config: GrafanaConfig) -> Self {
        self.grafana = Some(config);
        self
    }

    pub fn with_loki(mut self, config: LokiConfig) -> Self {
        self.loki = Some(config);
        self
    }
}
