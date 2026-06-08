//! Linux/macOS 采集器共享的纯计算逻辑。
//!
//! 这些 helper 只依赖采样值本身,不关心平台如何读取 `/proc`、sysctl 或 Mach API,
//! 因此集中在一个模块里避免跨平台实现再次复制并产生漂移。

use std::collections::VecDeque;
use std::time::Instant;

use nodelite_proto::percentage;

#[derive(Debug, Clone, Copy)]
pub(super) struct CpuSample {
    pub(super) total: u64,
    pub(super) idle: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NetworkSample {
    pub(super) observed_at: Instant,
    pub(super) rx_bytes: u64,
    pub(super) tx_bytes: u64,
    pub(super) rx_packets: u64,
    pub(super) tx_packets: u64,
    pub(super) rx_dropped_packets: u64,
    pub(super) tx_dropped_packets: u64,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NetworkTotals {
    pub(super) rx_bytes: u64,
    pub(super) tx_bytes: u64,
    pub(super) rx_packets: u64,
    pub(super) tx_packets: u64,
    pub(super) rx_dropped_packets: u64,
    pub(super) tx_dropped_packets: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct NetworkMetrics {
    pub(super) rx_bytes_per_sec: Option<f64>,
    pub(super) tx_bytes_per_sec: Option<f64>,
    pub(super) packet_loss_percent: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NetworkRateDirection {
    Rx,
    Tx,
}

impl NetworkRateDirection {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Rx => "rx",
            Self::Tx => "tx",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NetworkRateAnomaly {
    pub(super) direction: NetworkRateDirection,
    pub(super) rate_bytes_per_sec: f64,
    pub(super) baseline_avg_bytes_per_sec: f64,
    pub(super) effective_baseline_bytes_per_sec: f64,
    pub(super) multiplier: f64,
    pub(super) sample_count: usize,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct NetworkRateAnomalies {
    pub(super) rx: Option<NetworkRateAnomaly>,
    pub(super) tx: Option<NetworkRateAnomaly>,
}

#[derive(Debug, Default)]
pub(super) struct NetworkRateBaselines {
    rx: NetworkRateBaseline,
    tx: NetworkRateBaseline,
}

#[derive(Debug, Default)]
struct NetworkRateBaseline {
    samples: VecDeque<f64>,
}

const NETWORK_RATE_BASELINE_WINDOW: usize = 32;
const NETWORK_RATE_BASELINE_MIN_SAMPLES: usize = 8;
const NETWORK_RATE_ANOMALY_MULTIPLIER: f64 = 100.0;
const NETWORK_RATE_BASELINE_FLOOR_BYTES_PER_SEC: f64 = 64.0 * 1024.0;

pub(super) fn compute_cpu_usage(previous: CpuSample, current: CpuSample) -> f64 {
    let total_delta = current.total.saturating_sub(previous.total);
    let idle_delta = current.idle.saturating_sub(previous.idle);
    if total_delta == 0 {
        return 0.0;
    }
    let busy = total_delta.saturating_sub(idle_delta);
    percentage(busy, total_delta)
}

pub(super) fn compute_network_metrics(
    previous: NetworkSample,
    observed_at: Instant,
    current: NetworkTotals,
) -> NetworkMetrics {
    let elapsed = observed_at
        .duration_since(previous.observed_at)
        .as_secs_f64();
    if elapsed <= f64::EPSILON {
        return NetworkMetrics::default();
    }

    let rx_rate = (current.rx_bytes >= previous.rx_bytes)
        .then(|| (current.rx_bytes - previous.rx_bytes) as f64 / elapsed);
    let tx_rate = (current.tx_bytes >= previous.tx_bytes)
        .then(|| (current.tx_bytes - previous.tx_bytes) as f64 / elapsed);
    NetworkMetrics {
        rx_bytes_per_sec: rx_rate,
        tx_bytes_per_sec: tx_rate,
        packet_loss_percent: compute_packet_loss_percent(previous, current),
    }
}

fn compute_packet_loss_percent(previous: NetworkSample, current: NetworkTotals) -> Option<f64> {
    let rx_packets = current.rx_packets.checked_sub(previous.rx_packets)?;
    let tx_packets = current.tx_packets.checked_sub(previous.tx_packets)?;
    let rx_dropped = current
        .rx_dropped_packets
        .checked_sub(previous.rx_dropped_packets)?;
    let tx_dropped = current
        .tx_dropped_packets
        .checked_sub(previous.tx_dropped_packets)?;
    let delivered_packets = rx_packets.saturating_add(tx_packets);
    let dropped_packets = rx_dropped.saturating_add(tx_dropped);
    let attempted_packets = delivered_packets.saturating_add(dropped_packets);
    Some(percentage(dropped_packets, attempted_packets))
}

impl NetworkRateBaselines {
    pub(super) fn observe(
        &mut self,
        rx_bytes_per_sec: Option<f64>,
        tx_bytes_per_sec: Option<f64>,
    ) -> NetworkRateAnomalies {
        NetworkRateAnomalies {
            rx: self.rx.observe(NetworkRateDirection::Rx, rx_bytes_per_sec),
            tx: self.tx.observe(NetworkRateDirection::Tx, tx_bytes_per_sec),
        }
    }

    #[cfg(target_os = "macos")]
    pub(super) fn clear(&mut self) {
        self.rx.clear();
        self.tx.clear();
    }
}

impl NetworkRateBaseline {
    fn observe(
        &mut self,
        direction: NetworkRateDirection,
        rate_bytes_per_sec: Option<f64>,
    ) -> Option<NetworkRateAnomaly> {
        let rate = rate_bytes_per_sec?;
        if !rate.is_finite() || rate < 0.0 {
            return None;
        }

        let anomaly = self.anomaly(direction, rate);
        self.push(rate);
        anomaly
    }

    fn anomaly(
        &self,
        direction: NetworkRateDirection,
        rate_bytes_per_sec: f64,
    ) -> Option<NetworkRateAnomaly> {
        let sample_count = self.samples.len();
        if sample_count < NETWORK_RATE_BASELINE_MIN_SAMPLES {
            return None;
        }

        let baseline_avg = self.samples.iter().sum::<f64>() / sample_count as f64;
        let effective_baseline = baseline_avg.max(NETWORK_RATE_BASELINE_FLOOR_BYTES_PER_SEC);
        let threshold = effective_baseline * NETWORK_RATE_ANOMALY_MULTIPLIER;
        if rate_bytes_per_sec <= threshold {
            return None;
        }

        Some(NetworkRateAnomaly {
            direction,
            rate_bytes_per_sec,
            baseline_avg_bytes_per_sec: baseline_avg,
            effective_baseline_bytes_per_sec: effective_baseline,
            multiplier: rate_bytes_per_sec / effective_baseline,
            sample_count,
        })
    }

    fn push(&mut self, rate_bytes_per_sec: f64) {
        if self.samples.len() == NETWORK_RATE_BASELINE_WINDOW {
            self.samples.pop_front();
        }
        self.samples.push_back(rate_bytes_per_sec);
    }

    #[cfg(target_os = "macos")]
    fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        CpuSample, NetworkRateBaselines, NetworkRateDirection, NetworkSample, NetworkTotals,
        compute_cpu_usage, compute_network_metrics,
    };

    #[test]
    fn computes_cpu_usage_from_deltas() {
        let previous = CpuSample {
            total: 560,
            idle: 410,
        };
        let current = CpuSample {
            total: 680,
            idle: 440,
        };

        let usage = compute_cpu_usage(previous, current);
        assert!(usage > 70.0 && usage < 80.0);
    }

    #[test]
    fn computes_network_rates_from_deltas() {
        let previous = NetworkSample {
            observed_at: Instant::now() - Duration::from_secs(2),
            rx_bytes: 100,
            tx_bytes: 40,
            rx_packets: 10,
            tx_packets: 4,
            rx_dropped_packets: 0,
            tx_dropped_packets: 0,
        };
        let current = NetworkTotals {
            rx_bytes: 220,
            tx_bytes: 100,
            rx_packets: 22,
            tx_packets: 10,
            rx_dropped_packets: 0,
            tx_dropped_packets: 0,
        };

        let metrics = compute_network_metrics(previous, Instant::now(), current);
        assert!(
            metrics
                .rx_bytes_per_sec
                .expect("rx rate should be reported")
                > 50.0
        );
        assert!(
            metrics
                .tx_bytes_per_sec
                .expect("tx rate should be reported")
                > 20.0
        );
    }

    #[test]
    fn computes_packet_loss_from_packet_deltas() {
        let previous = NetworkSample {
            observed_at: Instant::now() - Duration::from_secs(2),
            rx_bytes: 100,
            tx_bytes: 40,
            rx_packets: 100,
            tx_packets: 50,
            rx_dropped_packets: 2,
            tx_dropped_packets: 3,
        };
        let current = NetworkTotals {
            rx_bytes: 220,
            tx_bytes: 100,
            rx_packets: 160,
            tx_packets: 80,
            rx_dropped_packets: 8,
            tx_dropped_packets: 7,
        };

        let metrics = compute_network_metrics(previous, Instant::now(), current);

        assert_eq!(metrics.packet_loss_percent, Some(10.0));
    }

    #[test]
    fn packet_loss_is_absent_when_packet_counters_reset() {
        let previous = NetworkSample {
            observed_at: Instant::now() - Duration::from_secs(2),
            rx_bytes: 100,
            tx_bytes: 40,
            rx_packets: 100,
            tx_packets: 50,
            rx_dropped_packets: 10,
            tx_dropped_packets: 0,
        };
        let current = NetworkTotals {
            rx_bytes: 220,
            tx_bytes: 100,
            rx_packets: 90,
            tx_packets: 60,
            rx_dropped_packets: 11,
            tx_dropped_packets: 0,
        };

        let metrics = compute_network_metrics(previous, Instant::now(), current);

        assert_eq!(metrics.packet_loss_percent, None);
    }

    #[test]
    fn network_rate_baseline_waits_for_enough_samples() {
        let mut baselines = NetworkRateBaselines::default();
        for _ in 0..7 {
            let anomalies = baselines.observe(Some(1_000_000.0), Some(1_000_000.0));
            assert_eq!(anomalies.rx, None);
            assert_eq!(anomalies.tx, None);
        }

        let anomalies = baselines.observe(Some(150_000_000.0), None);

        assert_eq!(anomalies.rx, None);
        assert_eq!(anomalies.tx, None);
    }

    #[test]
    fn network_rate_baseline_flags_100x_spikes_without_dropping_them() {
        let mut baselines = NetworkRateBaselines::default();
        for _ in 0..8 {
            baselines.observe(Some(1_000_000.0), None);
        }

        let first = baselines.observe(Some(150_000_000.0), None);
        let anomaly = first.rx.expect("spike should be marked");
        assert_eq!(anomaly.direction, NetworkRateDirection::Rx);
        assert_eq!(anomaly.sample_count, 8);
        assert_eq!(anomaly.baseline_avg_bytes_per_sec, 1_000_000.0);
        assert!(anomaly.multiplier >= 150.0);

        let second = baselines.observe(Some(150_000_000.0), None);
        assert_eq!(second.rx, None);
    }

    #[test]
    fn network_rate_baseline_uses_floor_for_idle_baselines() {
        let mut baselines = NetworkRateBaselines::default();
        for _ in 0..8 {
            baselines.observe(None, Some(0.0));
        }

        let quiet_burst = baselines.observe(None, Some(1_000_000.0));
        assert_eq!(quiet_burst.tx, None);

        let mut baselines = NetworkRateBaselines::default();
        for _ in 0..8 {
            baselines.observe(None, Some(0.0));
        }

        let large_burst = baselines.observe(None, Some(10_000_000.0));
        let anomaly = large_burst.tx.expect("large burst should be marked");
        assert_eq!(anomaly.direction, NetworkRateDirection::Tx);
        assert_eq!(anomaly.baseline_avg_bytes_per_sec, 0.0);
        assert_eq!(anomaly.effective_baseline_bytes_per_sec, 64.0 * 1024.0);
        assert!(anomaly.multiplier > 100.0);
    }
}
