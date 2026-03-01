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
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};

use crate::node_marker::NodeMarker;

/// Plugin that adds data ingestion functionality
pub struct DataIngestionPlugin;

impl Plugin for DataIngestionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NodeStatusBuffer>()
            .init_resource::<DataIngestionConfig>()
            .init_resource::<DataIngestionState>()
            .add_systems(Update, (poll_data_sources, apply_node_status_updates).chain());
    }
}

/// Configuration for data ingestion
#[derive(Resource)]
pub struct DataIngestionConfig {
    /// Poll interval in seconds
    pub poll_interval_secs: f32,
    /// Grafana connection configuration (None = disabled)
    pub grafana: Option<GrafanaConfig>,
    /// Prometheus query config (routed through Grafana)
    pub prometheus_query: Option<GrafanaPrometheusQuery>,
    /// Loki query config (routed through Grafana)
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

    let mut task = state.pending_task.take().unwrap();
    let nodes = block_on(poll_once(&mut task)).unwrap_or_else(|| {
        warn!("Data ingestion task returned no result despite being finished");
        Vec::new()
    });

    for node in nodes {
        if let Some(mut marker) = markers.iter_mut().find(|m| m.id == node.id) {
            marker.health = node.health;
        }
        buffer.update(node);
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
}
