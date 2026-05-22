//! WebSocket 入站会话处理。
//!
//! 从 [`ws_handler`](/ws 路由入口)进来后,流程是:
//! 1. 通过 `WsAdmissionController` 拿到连接配额(RAII permit);
//! 2. 升级到 WebSocket;
//! 3. `handle_socket` 接管单个会话:Hello → registry.authorize → 进入 Ping
//!    心跳 + Metrics 数据循环 + 主动 token 轮换 + Refresh 请求处理;
//! 4. 会话退出时 SharedState/连接计数自动回收。
//!
//! 这是 server 内部最大的一段状态机,把它放到独立模块,使 main.rs 只剩
//! "组装路由 + 启动后台任务"的骨架。

mod handshake;
mod protocol;
mod refresh;
mod session;

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use tracing::{error, warn};

use crate::AppState;
use crate::admission::{WsAdmissionError, resolve_client_ip, ws_admission_error_response};
use crate::audit::{AuditEventType, NewAuditEvent};

use self::handshake::handle_socket;
use self::protocol::ProtocolError;
#[cfg(test)]
use self::protocol::{
    ParsedFrame, encode_ping_message, parse_wire_message, prune_outstanding_pings,
};

struct ActiveSession {
    node_id: String,
    node_label: String,
    session_id: u64,
    session_token: String,
    session_generation: u64,
}

enum LoopAction {
    Continue,
    Break,
}

/// `/ws` 入口:在 WebSocket 升级前先做准入检查与帧大小限制。
pub async fn ws_handler(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let max_message_bytes = state.shared.config().max_message_bytes;
    let client_ip = resolve_client_ip(&state.shared.config().trusted_proxies, peer_addr, &headers);
    let audit_user_agent = header_user_agent(&headers);
    let connection_permit = match state.ws_admission.try_acquire(client_ip) {
        Ok(permit) => permit,
        Err(error) => {
            maybe_record_ws_block(&state, &error, client_ip, audit_user_agent.clone()).await;
            return ws_admission_error_response(error);
        }
    };
    ws.max_frame_size(max_message_bytes)
        .max_message_size(max_message_bytes)
        .on_upgrade(move |socket| async move {
            if let Err(error) = handle_socket(
                state,
                client_ip,
                audit_user_agent,
                connection_permit,
                socket,
            )
            .await
            {
                match error {
                    ProtocolError::Client(message) => {
                        warn!(reason = %message, "websocket client disconnected");
                    }
                    ProtocolError::Server(error) => {
                        error!(error = ?error, "websocket session failed");
                    }
                }
            }
        })
        .into_response()
}

async fn maybe_record_ws_block(
    state: &AppState,
    error: &WsAdmissionError,
    client_ip: IpAddr,
    user_agent: Option<String>,
) {
    let WsAdmissionError::Blocked { retry_after_secs } = error else {
        return;
    };
    let mut event = NewAuditEvent::now(
        AuditEventType::RateLimitExceeded,
        client_ip.to_string(),
        false,
    );
    event.user_agent = user_agent;
    event.details = json!({
        "endpoint": "/ws",
        "retry_after_secs": retry_after_secs,
        "reason": "websocket_auth_block",
    });
    state.audit_log.record_best_effort(event).await;
}

fn header_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use axum::extract::ws::Message;
    use nodelite_proto::{HelloMessage, NodeIdentity, WIRE_PROTOCOL_VERSION, WireMessage};

    use super::{
        ParsedFrame, ProtocolError, encode_ping_message, parse_wire_message,
        prune_outstanding_pings,
    };

    fn hello_text_frame() -> Message {
        let hello = WireMessage::Hello(HelloMessage {
            protocol_version: WIRE_PROTOCOL_VERSION,
            identity: NodeIdentity {
                node_id: "hk-01".to_string(),
                node_label: "Hong Kong 01".to_string(),
                hostname: "hk-01.internal".to_string(),
                os: "Linux".to_string(),
                kernel_version: None,
                cpu_model: None,
                cpu_cores: 2,
                agent_version: "0.1.0".to_string(),
                boot_time: None,
                tags: vec!["edge".to_string()],
            },
            token: "secret".to_string(),
        });
        Message::Text(
            serde_json::to_string(&hello)
                .expect("hello should serialize")
                .into(),
        )
    }

    #[test]
    fn encode_ping_message_matches_wire_protocol_shape() {
        assert_eq!(encode_ping_message(42), r#"{"type":"ping","nonce":42}"#);
    }

    #[test]
    fn parse_wire_message_decodes_text_frames() {
        let parsed = parse_wire_message(hello_text_frame()).expect("hello should parse");
        assert!(matches!(parsed, ParsedFrame::Wire(_)));
    }

    #[test]
    fn parse_wire_message_rejects_invalid_json() {
        let error = parse_wire_message(Message::Text("{not-json}".into()))
            .expect_err("invalid json should be rejected");
        assert!(
            matches!(error, ProtocolError::Client(message) if message.contains("invalid websocket json"))
        );
    }

    #[test]
    fn parse_wire_message_rejects_binary_frames() {
        let error = parse_wire_message(Message::Binary(vec![1, 2, 3].into()))
            .expect_err("binary frames should be rejected");
        assert!(
            matches!(error, ProtocolError::Client(message) if message == "binary websocket messages are not supported")
        );
    }

    #[test]
    fn parse_wire_message_treats_ping_as_control() {
        let parsed =
            parse_wire_message(Message::Ping(vec![1, 2, 3].into())).expect("ping should parse");
        assert!(matches!(parsed, ParsedFrame::Control));
    }

    #[test]
    fn parse_wire_message_treats_pong_as_control() {
        let parsed =
            parse_wire_message(Message::Pong(vec![4, 5, 6].into())).expect("pong should parse");
        assert!(matches!(parsed, ParsedFrame::Control));
    }

    #[test]
    fn parse_wire_message_treats_close_as_close() {
        let parsed = parse_wire_message(Message::Close(None)).expect("close should parse");
        assert!(matches!(parsed, ParsedFrame::Close));
    }

    #[test]
    fn prune_outstanding_pings_removes_expired_entries() {
        let now = Instant::now();
        let mut outstanding = HashMap::from([
            (1_u64, now - Duration::from_secs(30)),
            (2_u64, now - Duration::from_secs(2)),
        ]);

        prune_outstanding_pings(&mut outstanding, Duration::from_secs(5), 8);

        assert_eq!(outstanding.len(), 1);
        assert!(outstanding.contains_key(&2));
    }

    #[test]
    fn prune_outstanding_pings_keeps_fresh_entries_below_capacity() {
        let now = Instant::now();
        let mut outstanding = HashMap::from([
            (1_u64, now - Duration::from_secs(1)),
            (2_u64, now - Duration::from_secs(2)),
        ]);

        prune_outstanding_pings(&mut outstanding, Duration::from_secs(10), 3);

        assert_eq!(outstanding.len(), 2);
        assert!(outstanding.contains_key(&1));
        assert!(outstanding.contains_key(&2));
    }

    #[test]
    fn prune_outstanding_pings_drops_oldest_entry_at_capacity() {
        let now = Instant::now();
        let mut outstanding = HashMap::from([
            (1_u64, now - Duration::from_secs(1)),
            (2_u64, now - Duration::from_secs(4)),
            (3_u64, now - Duration::from_secs(2)),
        ]);

        prune_outstanding_pings(&mut outstanding, Duration::from_secs(10), 3);

        assert_eq!(outstanding.len(), 2);
        assert!(!outstanding.contains_key(&2));
        assert!(outstanding.contains_key(&1));
        assert!(outstanding.contains_key(&3));
    }
}
