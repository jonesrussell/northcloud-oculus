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
