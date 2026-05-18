//! 覆盖 #29 的优雅停机语义:CancellationToken 触发后,
//! 服务端应当向活跃 WebSocket 会话主动发送 Close(1001 "going away"),
//! 而不是任凭 TCP 连接被 drop。
//!
//! Agent 收到带原因的 Close 帧后可以走"已知关停"分支退避,
//! 避免重连风暴 (#27 在退避层做的事情在这里得到协议层支撑)。

use std::time::Duration;

use super::{Result, TEST_TIMEOUT, TestAgent, TestServer};

#[tokio::test]
async fn websocket_session_receives_close_frame_on_shutdown() -> Result<()> {
    let server = TestServer::start().await?;
    let node = server.issue_node("hk-01", "Hong Kong").await?;
    let mut agent = TestAgent::connect(&server, &node).await?;

    // Agent 已经握手认证完毕,模拟"SIGTERM 触发进程级关停":
    // production 中由 shutdown_signal() → run_server 完成 token.cancel(),
    // 测试这里直接 cancel 同一个 token。
    server.cancel_shutdown();

    let (code, reason) = agent.wait_for_close_frame(TEST_TIMEOUT).await?;

    // 1001 = "going away",WebSocket RFC 6455 规定的"对端要离开"语义,
    // 这是 agent 端识别"服务端主动重启"vs"网络断"的唯一信号。
    assert_eq!(code, 1001, "expected close code AWAY (1001), got {code}");
    assert!(
        reason.contains("shut") || reason.contains("server"),
        "expected reason to mention shutdown/server, got {reason:?}"
    );

    // server 应当真的能在合理时间内退出 —— 否则后台任务 leak 或者 axum
    // graceful 不响应 cancellation 都会让这步 hang。
    tokio::time::timeout(Duration::from_secs(5), server.shutdown())
        .await
        .map_err(|_| anyhow::anyhow!("server did not shut down within 5s after cancel"))??;

    Ok(())
}
