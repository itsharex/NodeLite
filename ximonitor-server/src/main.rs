mod state;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use futures::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::fs;
use tokio::net::TcpListener;
use tokio::time::interval;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use ximonitor_proto::{
    HelloMessage, MetricsMessage, NodeIdentity, PingMessage, PongMessage, ServerConfig,
    ServerNoticeMessage, WireMessage, parse_server_config,
};

use crate::state::SharedState;

#[derive(Debug, Parser)]
#[command(name = "ximonitor-server")]
#[command(about = "XiMonitor central server")]
struct Cli {
    #[arg(long, default_value = "config/server.toml")]
    config: PathBuf,
}

#[derive(Clone)]
struct AppState {
    shared: SharedState,
}

#[derive(Debug, Serialize)]
struct BootstrapResponse {
    service: &'static str,
    status: &'static str,
    public_base_url: String,
    refresh_interval_secs: u64,
    registered_nodes: usize,
}

#[derive(Debug)]
enum ProtocolError {
    Client(String),
    Server(anyhow::Error),
}

#[derive(Debug)]
enum ParsedFrame {
    Wire(WireMessage),
    Control,
    Close,
}

impl From<anyhow::Error> for ProtocolError {
    fn from(error: anyhow::Error) -> Self {
        Self::Server(error)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = Arc::new(load_server_config(&cli.config).await?);
    let listen_addr = config.listen;
    let public_base_url = config.public_base_url.clone();
    let refresh_interval_secs = config.refresh_interval_secs;
    let shared = SharedState::new(Arc::clone(&config));

    spawn_stale_reaper(shared.clone());

    let state = AppState { shared };
    let app = Router::new()
        .route("/", get(index))
        .route("/nodes/:node_id", get(node_detail))
        .route("/healthz", get(healthz))
        .route("/api/bootstrap", get(bootstrap))
        .route("/ws", get(ws_handler))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("failed to bind server listener to {listen_addr}"))?;

    info!(
        listen = %listen_addr,
        public_base_url = %public_base_url,
        refresh_interval_secs,
        "ximonitor server listening",
    );

    axum::serve(listener, app)
        .await
        .context("server exited unexpectedly")
}

async fn load_server_config(path: &Path) -> Result<ServerConfig> {
    let content = fs::read_to_string(path)
        .await
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let config = parse_server_config(&content)
        .map_err(|error| anyhow!("failed to parse {}: {error}", path.display()))?;

    if let Some(parent) = config.snapshot_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            warn!(
                snapshot_dir = %parent.display(),
                "snapshot directory does not exist yet; it will be created later",
            );
        }
    }
    if let Some(parent) = config.history_db_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            warn!(
                history_dir = %parent.display(),
                "history directory does not exist yet; it will be created later",
            );
        }
    }

    Ok(config)
}

async fn index() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>XiMonitor</title>
    <style>
      :root {
        color-scheme: light;
        font-family: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", serif;
        background: linear-gradient(135deg, #f3efe3 0%, #f9f7f1 55%, #e9f0f5 100%);
        color: #17212b;
      }
      body {
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
      }
      main {
        width: min(720px, calc(100vw - 32px));
        background: rgba(255, 255, 255, 0.82);
        border: 1px solid rgba(23, 33, 43, 0.08);
        border-radius: 24px;
        padding: 40px;
        box-shadow: 0 24px 80px rgba(23, 33, 43, 0.12);
        backdrop-filter: blur(20px);
      }
      h1 {
        font-size: clamp(2.5rem, 6vw, 4rem);
        line-height: 0.95;
        margin: 0 0 12px;
      }
      p {
        font-size: 1.1rem;
        line-height: 1.7;
        margin: 0 0 12px;
      }
      code {
        font-family: "SFMono-Regular", "SF Mono", ui-monospace, monospace;
      }
    </style>
  </head>
  <body>
    <main>
      <h1>XiMonitor</h1>
      <p>WebSocket ingress is live. Agents can authenticate with a <code>hello</code> message and then stream <code>metrics</code>.</p>
      <p>Dashboard APIs and node details land in the next commits.</p>
      <p>Node placeholder route: <code>/nodes/&lt;node_id&gt;</code>.</p>
    </main>
  </body>
</html>"#,
    )
}

async fn node_detail(AxumPath(node_id): AxumPath<String>) -> Html<String> {
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>XiMonitor Node</title></head><body><main><h1>{}</h1><p>Read-only node details will be rendered here in the next commit.</p></main></body></html>",
        html_escape(&node_id)
    ))
}

async fn healthz() -> StatusCode {
    StatusCode::OK
}

async fn bootstrap(State(state): State<AppState>) -> impl IntoResponse {
    Json(BootstrapResponse {
        service: "ximonitor-server",
        status: "ok",
        public_base_url: state.shared.config().public_base_url.clone(),
        refresh_interval_secs: state.shared.config().refresh_interval_secs,
        registered_nodes: state.shared.node_count().await,
    })
}

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    let max_message_bytes = state.shared.config().max_message_bytes;
    ws.max_frame_size(max_message_bytes)
        .max_message_size(max_message_bytes)
        .on_upgrade(move |socket| async move {
            if let Err(error) = handle_socket(state.shared, socket).await {
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
}

async fn handle_socket(shared: SharedState, mut socket: WebSocket) -> Result<(), ProtocolError> {
    let hello = recv_hello(&mut socket).await?;
    validate_hello(shared.config(), &hello)?;

    let node_id = hello.identity.node_id.clone();
    let node_label = hello.identity.node_label.clone();
    let session_id = shared.register_node(hello.identity).await;

    info!(node_id = %node_id, node_label = %node_label, session_id, "node authenticated");

    let notice = WireMessage::ServerNotice(ServerNoticeMessage {
        level: ximonitor_proto::NoticeLevel::Info,
        message: "authenticated".to_string(),
    });
    send_wire_message(&mut socket, &notice).await?;

    let (mut sender, mut receiver) = socket.split();
    let ping_every = Duration::from_secs(shared.config().ping_interval_secs);
    let mut ping_ticker = interval(ping_every);
    let mut outstanding_pings: HashMap<u64, Instant> = HashMap::new();
    let mut next_ping_nonce = 1_u64;

    let session_result: Result<(), ProtocolError> = loop {
        tokio::select! {
            incoming = receiver.next() => {
                let Some(frame) = incoming else {
                    break Ok(());
                };
                let frame = frame.map_err(|error| anyhow!("websocket receive failed: {error}"))?;

                match parse_wire_message(frame)? {
                    ParsedFrame::Close => break Ok(()),
                    ParsedFrame::Control => continue,
                    ParsedFrame::Wire(WireMessage::Metrics(MetricsMessage { snapshot })) => {
                        if !shared.update_snapshot(&node_id, session_id, snapshot).await {
                            warn!(node_id = %node_id, session_id, "dropping metrics from superseded session");
                            break Ok(());
                        }
                    }
                    ParsedFrame::Wire(WireMessage::Pong(PongMessage { nonce })) => {
                        let Some(sent_at) = outstanding_pings.remove(&nonce) else {
                            continue;
                        };
                        let latency_ms = sent_at.elapsed().as_millis() as u64;
                        if !shared.update_latency(&node_id, session_id, latency_ms).await {
                            warn!(node_id = %node_id, session_id, "dropping pong from superseded session");
                            break Ok(());
                        }
                    }
                    ParsedFrame::Wire(WireMessage::Hello(_)) => {
                        break Err(ProtocolError::Client("duplicate hello message".to_string()));
                    }
                    ParsedFrame::Wire(WireMessage::Ping(_)) => {
                        break Err(ProtocolError::Client("agent must not send ping messages".to_string()));
                    }
                    ParsedFrame::Wire(WireMessage::ServerNotice(_)) => {
                        break Err(ProtocolError::Client("agent must not send server_notice messages".to_string()));
                    }
                }
            }
            _ = ping_ticker.tick() => {
                if !shared.is_current_session(&node_id, session_id).await {
                    warn!(node_id = %node_id, session_id, "closing superseded websocket session");
                    break Ok(());
                }

                let nonce = next_ping_nonce;
                next_ping_nonce = next_ping_nonce.saturating_add(1);
                outstanding_pings.insert(nonce, Instant::now());
                let ping = serde_json::to_string(&WireMessage::Ping(PingMessage { nonce }))
                    .map_err(|error| anyhow!("failed to serialize ping: {error}"))?;
                sender
                    .send(Message::Text(ping.into()))
                    .await
                    .map_err(|error| anyhow!("failed to send ping: {error}"))?;
            }
        }
    };

    shared.mark_disconnected(&node_id, session_id).await;
    info!(node_id = %node_id, session_id, "node disconnected");
    session_result
}

async fn recv_hello(socket: &mut WebSocket) -> Result<HelloMessage, ProtocolError> {
    loop {
        let Some(message) = socket
            .recv()
            .await
            .transpose()
            .map_err(|error| anyhow!("failed to receive hello: {error}"))?
        else {
            return Err(ProtocolError::Client(
                "connection closed before hello message".to_string(),
            ));
        };

        match parse_wire_message(message)? {
            ParsedFrame::Control => continue,
            ParsedFrame::Wire(WireMessage::Hello(hello)) => return Ok(hello),
            ParsedFrame::Wire(_) => {
                return Err(ProtocolError::Client(
                    "first websocket message must be hello".to_string(),
                ));
            }
            ParsedFrame::Close => {
                return Err(ProtocolError::Client(
                    "connection closed before hello message".to_string(),
                ));
            }
        }
    }
}

fn validate_hello(config: &ServerConfig, hello: &HelloMessage) -> Result<(), ProtocolError> {
    if hello.token != config.shared_token {
        return Err(ProtocolError::Client("invalid shared token".to_string()));
    }
    validate_identity(&hello.identity)
}

fn validate_identity(identity: &NodeIdentity) -> Result<(), ProtocolError> {
    if identity.node_id.trim().is_empty() {
        return Err(ProtocolError::Client(
            "identity.node_id is empty".to_string(),
        ));
    }
    if identity.node_label.trim().is_empty() {
        return Err(ProtocolError::Client(
            "identity.node_label is empty".to_string(),
        ));
    }
    if identity.agent_version.trim().is_empty() {
        return Err(ProtocolError::Client(
            "identity.agent_version is empty".to_string(),
        ));
    }
    Ok(())
}

fn parse_wire_message(message: Message) -> Result<ParsedFrame, ProtocolError> {
    match message {
        Message::Text(text) => serde_json::from_str::<WireMessage>(&text)
            .map(ParsedFrame::Wire)
            .map_err(|error| ProtocolError::Client(format!("invalid websocket json: {error}"))),
        Message::Binary(_) => Err(ProtocolError::Client(
            "binary websocket messages are not supported".to_string(),
        )),
        Message::Close(_) => Ok(ParsedFrame::Close),
        Message::Ping(_) | Message::Pong(_) => Ok(ParsedFrame::Control),
    }
}

async fn send_wire_message(
    socket: &mut WebSocket,
    message: &WireMessage,
) -> Result<(), ProtocolError> {
    let payload = serde_json::to_string(message)
        .map_err(|error| anyhow!("failed to serialize websocket message: {error}"))?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| anyhow!("failed to send websocket message: {error}"))?;
    Ok(())
}

fn spawn_stale_reaper(shared: SharedState) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let count = shared.mark_stale().await;
            if count > 0 {
                info!(count, "marked stale nodes offline");
            }
        }
    });
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ximonitor_server=info,tower_http=info".into()),
        )
        .with_target(false)
        .compact()
        .init();
}
