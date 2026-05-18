use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::admission::{InstallAdmissionController, WsAdmissionController};
use crate::agent_logs::AgentLogStore;
use crate::auth::{ReadonlyRouteAuth, TwoFactorSessions};
use crate::history::HistoryStore;
use crate::registry::NodeRegistry;
use crate::state::SharedState;

/// 在各处理器之间共享的运行时上下文。
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) history: HistoryStore,
    pub(crate) agent_logs: AgentLogStore,
    pub(crate) install_admission: InstallAdmissionController,
    /// `/api/verify-2fa` 的 IP 维度限流器:与 `install_admission` 同型,
    /// 但实例独立,避免安装接口的失败计数误伤 2FA 登录,反之亦然。
    pub(crate) verify_2fa_admission: InstallAdmissionController,
    pub(crate) readiness: ServerReadiness,
    pub(crate) registry: NodeRegistry,
    pub(crate) shared: SharedState,
    pub(crate) ws_admission: WsAdmissionController,
    pub(crate) readonly_auth: Arc<RwLock<ReadonlyRouteAuth>>,
    pub(crate) two_factor_sessions: TwoFactorSessions,
    pub(crate) config_path: Arc<PathBuf>,
    /// 进程级关停信号。axum graceful shutdown 之后由 `run_server` 触发,
    /// 所有后台任务与活跃 WS 会话都订阅此 token 以协同退出。
    pub(crate) shutdown: CancellationToken,
}

/// 只跟踪"对外是否可服务"所需的几个关键依赖状态。
///
/// - `healthz` 仍然只回答"进程是否存活";
/// - `readyz` 与 `/api/bootstrap.status` 则用这里的状态反映"是否已具备对外服务能力"。
#[derive(Clone)]
pub(crate) struct ServerReadiness {
    history_available: Arc<AtomicBool>,
    registry_reload_healthy: Arc<AtomicBool>,
}

impl ServerReadiness {
    pub(crate) fn new(history_available: bool) -> Self {
        Self {
            history_available: Arc::new(AtomicBool::new(history_available)),
            registry_reload_healthy: Arc::new(AtomicBool::new(true)),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.history_available() && self.registry_reload_healthy()
    }

    pub(crate) fn status_label(&self) -> &'static str {
        if self.is_ready() { "ok" } else { "degraded" }
    }

    pub(crate) fn history_available(&self) -> bool {
        self.history_available.load(Ordering::Relaxed)
    }

    pub(crate) fn registry_reload_healthy(&self) -> bool {
        self.registry_reload_healthy.load(Ordering::Relaxed)
    }

    pub(crate) fn mark_history_available(&self, available: bool) {
        self.history_available.store(available, Ordering::Relaxed);
    }

    pub(crate) fn mark_registry_reload_healthy(&self, healthy: bool) {
        self.registry_reload_healthy
            .store(healthy, Ordering::Relaxed);
    }
}
