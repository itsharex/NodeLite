//! 概览 API 与 Prometheus 输出的瞬时缓存。

use std::time::{Duration, Instant};

use axum::body::Bytes;

use crate::ServerReadiness;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReadinessSnapshot {
    ready: bool,
    history_available: bool,
    registry_reload_healthy: bool,
}

impl ReadinessSnapshot {
    pub(super) fn new(ready: bool, history_available: bool, registry_reload_healthy: bool) -> Self {
        Self {
            ready,
            history_available,
            registry_reload_healthy,
        }
    }

    pub(super) fn capture(readiness: &ServerReadiness) -> Self {
        Self::new(
            readiness.is_ready(),
            readiness.history_available(),
            readiness.registry_reload_healthy(),
        )
    }
}

/// 简单 JSON 视图(overview / nodes)的缓存槽:仅按 revision 校验。
#[derive(Debug, Default)]
pub(super) struct JsonViewSlot {
    revision: u64,
    body: Option<Bytes>,
}

impl JsonViewSlot {
    pub(super) fn get(&self, revision: u64) -> Option<Bytes> {
        if self.revision != revision {
            return None;
        }
        self.body.clone()
    }

    pub(super) fn store(&mut self, revision: u64, body: Bytes) {
        self.revision = revision;
        self.body = Some(body);
    }
}

/// Prometheus `/metrics` 文本的缓存槽:revision、readiness 与 TTL 三重校验。
#[derive(Debug, Default)]
pub(super) struct MetricsViewSlot {
    revision: u64,
    readiness: Option<ReadinessSnapshot>,
    cached_at: Option<Instant>,
    body: Option<Bytes>,
}

impl MetricsViewSlot {
    pub(super) fn get(
        &self,
        revision: u64,
        readiness: ReadinessSnapshot,
        max_age: Duration,
    ) -> Option<Bytes> {
        if self.revision != revision {
            return None;
        }
        if self.readiness != Some(readiness) {
            return None;
        }
        if self
            .cached_at
            .is_none_or(|cached_at| cached_at.elapsed() > max_age)
        {
            return None;
        }
        self.body.clone()
    }

    pub(super) fn store(&mut self, revision: u64, readiness: ReadinessSnapshot, body: Bytes) {
        self.revision = revision;
        self.readiness = Some(readiness);
        self.cached_at = Some(Instant::now());
        self.body = Some(body);
    }
}

