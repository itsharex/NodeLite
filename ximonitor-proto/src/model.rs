use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeIdentity {
    pub node_id: String,
    pub node_label: String,
    pub hostname: String,
    pub os: String,
    pub kernel_version: Option<String>,
    pub cpu_model: Option<String>,
    pub cpu_cores: u32,
    pub agent_version: String,
    pub boot_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryUsage {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiskUsage {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkCounters {
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub rx_bytes_per_sec: Option<f64>,
    pub tx_bytes_per_sec: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeSnapshot {
    pub collected_at: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub load: LoadAverage,
    pub memory: MemoryUsage,
    pub uptime_secs: u64,
    #[serde(default)]
    pub disks: Vec<DiskUsage>,
    pub network: NetworkCounters,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeStatus {
    pub identity: NodeIdentity,
    pub snapshot: Option<NodeSnapshot>,
    pub last_seen: Option<DateTime<Utc>>,
    pub latency_ms: Option<u64>,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryPoint {
    pub node_id: String,
    pub recorded_at: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub memory_used_percent: f64,
    pub rx_bytes_per_sec: Option<f64>,
    pub tx_bytes_per_sec: Option<f64>,
    pub latency_ms: Option<u64>,
    pub disk_used_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OverviewData {
    pub generated_at: DateTime<Utc>,
    pub total_nodes: usize,
    pub online_nodes: usize,
    pub offline_nodes: usize,
    pub total_rx_bytes: u64,
    pub total_tx_bytes: u64,
    pub current_rx_bytes_per_sec: f64,
    pub current_tx_bytes_per_sec: f64,
    pub average_latency_ms: Option<f64>,
}

impl MemoryUsage {
    pub fn used_percent(&self) -> f64 {
        percentage(self.used_bytes, self.total_bytes)
    }

    pub fn swap_used_percent(&self) -> Option<f64> {
        (self.swap_total_bytes > 0).then(|| percentage(self.swap_used_bytes, self.swap_total_bytes))
    }
}

pub fn percentage(used: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (used as f64 / total as f64) * 100.0
}
