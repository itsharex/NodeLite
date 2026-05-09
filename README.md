# XiMonitor

XiMonitor 是一个用 Rust 编写的轻量级服务器监控面板，包含：

- `ximonitor-server`
  中心服务，提供 WebSocket 接入、只读页面、只读 JSON API、SQLite 短期历史和快照恢复。
- `ximonitor-agent`
  Linux agent，采集 CPU、负载、内存、磁盘、网络总流量、实时速率和 WebSocket RTT。
- `ximonitor-proto`
  服务端与 agent 共用的配置、协议和数据模型。

## 当前能力

- 服务端只读页面：
  - `/`
  - `/nodes/:node_id`
- 服务端只读 API：
  - `/api/overview`
  - `/api/nodes`
  - `/api/nodes/:node_id`
  - `/api/nodes/:node_id/history`
- agent 接入协议：
  - `hello`
  - `metrics`
  - `ping`
  - `pong`
  - `server_notice`
- 72 小时 SQLite 历史保留
- 快照落盘与进程重启后恢复最近状态
- agent 指数退避自动重连

## 本地构建

```bash
cargo check
```

## 交叉编译 Linux x86_64

仓库内已经包含 musl 目标的 `lld` 链接配置，可以直接在 macOS 上构建静态 Linux `x86_64` 产物：

```bash
cargo build --release --target x86_64-unknown-linux-musl \
  -p ximonitor-server \
  -p ximonitor-agent
```

产物位置：

```bash
target/x86_64-unknown-linux-musl/release/ximonitor-server
target/x86_64-unknown-linux-musl/release/ximonitor-agent
```

这两个产物是：

- ELF 64-bit
- x86-64
- statically linked
- stripped

## 服务端启动

1. 复制配置：

```bash
mkdir -p config
cp config/server.example.toml config/server.toml
```

2. 修改 `config/server.toml` 中的 `shared_token` 和监听地址。

3. 启动服务端：

```bash
cargo run -p ximonitor-server -- --config config/server.toml
```

## Agent 启动

1. 复制配置：

```bash
cp config/agent.example.toml config/agent.toml
```

2. 修改 `node_id`、`node_label`、`server`、`token`。

3. 本机采样自检：

```bash
cargo run -p ximonitor-agent -- --config config/agent.toml --sample-once
```

4. 正常运行：

```bash
cargo run -p ximonitor-agent -- --config config/agent.toml
```

## 一键安装

脚本位置：

```bash
scripts/install-agent.sh
```

示例：

```bash
curl -fsSL https://your-host/install-agent.sh | sh -s -- \
  --server ws://monitor.example.com:8080/ws \
  --node-id hk-01 \
  --token YOUR_TOKEN \
  --base-url https://your-host/releases/latest/download
```

说明：

- 脚本会检测架构并下载对应的 `ximonitor-agent-<target>` 二进制
- 会写入 `/etc/ximonitor/agent.toml`
- 会生成 `ximonitor-agent.service`
- 会执行 `daemon-reload`、`enable` 和 `restart`

如果你已经有精确二进制地址，也可以改用：

```bash
sh scripts/install-agent.sh \
  --server ws://monitor.example.com:8080/ws \
  --node-id hk-01 \
  --token YOUR_TOKEN \
  --binary-url https://your-host/releases/ximonitor-agent-x86_64-unknown-linux-musl
```

## 说明

- 网页端默认只读，不提供写配置入口。
- 首版 agent 只支持 Linux。
- 当前历史图保存基础趋势，不做长期归档。
- 生产环境建议放在 Nginx 或 Caddy 后面并启用 HTTPS。
