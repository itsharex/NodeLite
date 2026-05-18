# NodeLite 安全架构

## 当前认证机制

### 1. Web UI 认证 (HTTP Basic Auth)

**实现位置**: `nodelite-server/src/handlers.rs` (Basic Auth 中间件)

**机制**:
- 使用 HTTP Basic Authentication (RFC 7617)
- 用户名/密码通过环境变量 `READONLY_USERNAME` / `READONLY_PASSWORD` 配置
- 服务器预计算 `Authorization: Basic <base64(username:password)>` 头
- 每次请求通过字符串比较验证 (constant-time comparison via `==`)

**保护范围**:
- `/` (主页)
- `/nodes/{node_id}` (节点详情页)
- `/api/bootstrap` (前端启动元数据)
- `/api/overview` (概览数据)
- `/api/nodes` (节点列表)
- `/api/nodes/{node_id}` (节点状态)
- `/api/nodes/{node_id}/history` (历史数据)

**会话管理**:
- **NEW (1.2.10)**: 前端 localStorage 存储认证时间戳
- **NEW (1.2.10)**: 24 小时后自动跳转到 `/logout-and-reauth` 强制重新登录
- 浏览器原生缓存 Basic Auth 凭据(关闭标签页后失效)

**安全特性**:
✅ 传输层必须使用 HTTPS (生产环境强制要求)
✅ 密码通过环境变量配置,不硬编码
✅ 预计算期望的 Authorization 头,避免每次请求重新编码
✅ 24 小时会话过期(1.2.10 新增)
✅ 未启用认证时直接放行(开发环境友好)

**双因素认证（2FA）**:
- 基于 TOTP（RFC 6238）
- 支持 Google Authenticator、Microsoft Authenticator
- 强制 HTTPS（启用 2FA 时）
- 安全措施：
  - 重放保护：同一 30 秒窗口内的 code 不允许重复使用
  - 暴力破解防护：连续 5 次验证失败后 pending session 失效
  - 常量时间比较：所有 TOTP 验证使用 `subtle::ConstantTimeEq`

**密码强度要求**:
- 最小长度：12 字符
- 必须包含：大小写字母、数字、特殊字符
- 禁止常见弱密码（top 10000）
- 服务端强制验证
- 错误信息包含具体缺失项

**潜在风险**:
⚠️ HTTP Basic Auth 凭据在每次请求中传输(base64 编码,非加密)
⚠️ 无密码强度要求(由运维人员自行保证)
⚠️ 无账号锁定机制(暴力破解防护依赖 rate limiting)
⚠️ 无审计日志(认证失败不记录)

---

### 2. Agent 认证 (WebSocket Token-Based)

**实现位置**: `nodelite-server/src/ws.rs:105-426`

**机制**:
- Agent 通过 WebSocket 连接到 `/ws`
- 握手后 10 秒内必须发送 `Hello` 消息,包含:
  - `identity.node_id`: 节点 ID
  - `identity.node_label`: 节点标签
  - `token`: 预共享密钥(由 `server issue-node` 生成)
- 服务器调用 `registry.authorize(&identity, &token)` 验证
- 验证通过后建立会话,失败则断开连接

**Token 生成**:
- 位置: `nodelite-server/src/registry.rs:issue_node`
- 算法: 32 字节随机数 → hex 编码 = 64 字符 token
- 熵源: `getrandom` crate (系统 CSPRNG)
- 存储: `server.json` (权限 0600)

**保护机制**:
✅ Token 由 CSPRNG 生成,128 位熵(暴力破解不可行)
✅ Token 与 node_id 绑定,无法跨节点复用
✅ 认证失败触发 IP 级别的速率限制(见下文)
✅ 握手超时 10 秒(防止慢速攻击)
✅ `server.json` 权限强制 0600(仅 root 可读)

**Token 过期机制**:
- 默认有效期：30 天（`DEFAULT_TOKEN_VALIDITY_DAYS`）
- 自动续期：距离过期不足 7 天时自动续期
- 续期触发：已认证会话内每次消息处理
- 续期流程：
  1. Agent 连接时携带当前 token
  2. 服务端验证 token 有效性
  3. 如果 token 距离过期 < 7 天，生成新 token
  4. 通过 `refreshed` 消息下发新 token
  5. Agent 更新本地配置文件

**潜在风险**:
⚠️ Token 无过期时间(一旦泄露永久有效,直到手动 rotate)
⚠️ 无 Token 撤销机制(除了删除节点或 rotate)
⚠️ 传输层未强制 TLS(开发环境可用 ws://,生产环境应强制 wss://)

---

### 3. 速率限制与防护

**WebSocket 准入控制** (`WsAdmissionController`):

**总连接数限制**:
- 默认: 1000 个并发 WebSocket 连接
- 超出返回 503 Service Unavailable

**单 IP 连接数限制**:
- 默认: 每个 IP 最多 10 个并发连接
- 超出返回 429 Too Many Requests

**认证失败封禁**:
- 窗口: 300 秒(5 分钟)
- 阈值: 5 次失败
- 惩罚: 封禁 900 秒(15 分钟)
- 返回: 429 Too Many Requests + `Retry-After` 头

**实现位置**: `nodelite-server/src/admission.rs`

**安全特性**:
✅ IP 级别速率限制(防止单一攻击者暴力破解)
✅ 指数退避(认证失败越多,封禁时间越长)
✅ 自动清理过期记录(避免内存泄漏)
✅ RAII 连接许可(连接断开自动释放配额)

**潜在风险**:
⚠️ 基于 IP 的限制可被 NAT 后的多个攻击者绕过
⚠️ 无全局速率限制(分布式攻击可能绕过单 IP 限制)
⚠️ 封禁状态存储在内存(服务重启后清空)

---

## 安全最佳实践

### 生产环境部署检查清单

#### 1. 传输层安全
- [ ] **强制 HTTPS**: 在反向代理(Nginx/Caddy)层终止 TLS
- [ ] **TLS 1.3**: 禁用 TLS 1.0/1.1,推荐 TLS 1.3
- [ ] **强密码套件**: 仅启用 AEAD 密码套件(如 AES-GCM, ChaCha20-Poly1305)
- [ ] **HSTS**: 启用 `Strict-Transport-Security` 头(max-age=31536000; includeSubDomains)
- [ ] **证书**: 使用受信任 CA 签发的证书(Let's Encrypt / 企业 CA)

#### 2. 认证配置
- [ ] **强密码**: `READONLY_PASSWORD` 至少 16 字符,包含大小写字母/数字/符号
- [ ] **密钥轮换**: 定期(每 90 天)更换 Web UI 密码
- [ ] **Token 轮换**: 定期(每 180 天)轮换 Agent token (`server issue-node --rotate-token`)
- [ ] **最小权限**: 仅授权必要的节点,及时删除下线节点

#### 3. 网络隔离
- [ ] **防火墙**: 仅开放必要端口(443/HTTPS, 不对外暴露 WebSocket 端口)
- [ ] **反向代理**: 通过 Nginx/Caddy 代理,隐藏后端服务
- [ ] **内网隔离**: Agent 与 Server 之间使用 VPN/专线,不走公网
- [ ] **IP 白名单**: 在反向代理层限制 Web UI 访问 IP 范围

#### 4. 监控与审计
- [ ] **日志记录**: 启用 `RUST_LOG=info`,记录所有认证失败事件
- [ ] **日志聚合**: 将日志发送到 SIEM 系统(如 ELK, Splunk)
- [ ] **告警**: 对异常认证失败率(>10/min)设置告警
- [ ] **定期审查**: 每月审查 `server.json` 中的节点列表,删除无效节点

#### 5. 系统加固
- [ ] **文件权限**: 确保 `server.json` 权限为 0600,所有者为 root
- [ ] **进程隔离**: 使用 systemd 的 `DynamicUser=yes` 或专用用户运行
- [ ] **资源限制**: 设置 `LimitNOFILE`, `LimitNPROC` 防止资源耗尽
- [ ] **SELinux/AppArmor**: 启用强制访问控制(MAC)

---

## 已知限制与缓解措施

### 限制 1: HTTP Basic Auth 凭据在每次请求中传输

**风险**: 中间人攻击可截获 base64 编码的凭据

**缓解**:
- **强制 HTTPS**: 在生产环境中,所有流量必须通过 TLS 加密
- **HSTS**: 防止降级攻击
- **证书固定**: 客户端(如移动 App)可固定服务器证书

**未来改进**:
- 考虑迁移到 JWT + Cookie 的会话机制
- 实现 OAuth 2.0 / OIDC 集成(企业 SSO)

### 限制 2: Agent Token 过期机制

**风险**: Token 泄露后在有效期内仍可使用

**当前实现**:
- Token 默认有效期：30 天
- 自动续期：距离过期不足 7 天时自动续期
- 续期触发：已认证会话内每次消息处理

**缓解**:
- **定期轮换**: 每 180 天手动执行 `server issue-node --rotate-token`
- **监控异常**: 对单个 Token 的异常连接频率设置告警
- **网络隔离**: Agent 与 Server 之间使用 VPN,减少 Token 泄露风险

**未来改进**:
- 支持 Token 撤销列表(Revocation List)
- 实现短期 Token + Refresh Token 机制

### 限制 3: 基于 IP 的速率限制可被绕过

**风险**: NAT 后的多个攻击者共享 IP,或分布式攻击使用多个 IP

**缓解**:
- **反向代理层限制**: 在 Nginx/Caddy 层实现全局速率限制
- **WAF**: 使用 Cloudflare / AWS WAF 防护
- **行为分析**: 监控异常流量模式(如短时间内大量不同 node_id 的认证失败)

**未来改进**:
- 实现全局速率限制(跨所有 IP)
- 集成 fail2ban 自动封禁攻击 IP
- 实现 CAPTCHA 挑战(Web UI)

---

## 安全事件响应

### 场景 1: Web UI 密码泄露

**响应步骤**:
1. 立即更换 `READONLY_PASSWORD` 环境变量
2. 重启 ximonitor-server 服务
3. 审查访问日志,确认是否有未授权访问
4. 通知相关人员,调查泄露原因

### 场景 2: Agent Token 泄露

**响应步骤**:
1. 执行 `server issue-node <node_id> --rotate-token` 轮换受影响节点的 Token
2. 在受影响的 Agent 上更新 Token(重新运行 install-agent.sh 或手动更新配置)
3. 审查 WebSocket 连接日志,确认是否有未授权连接
4. 如果多个 Token 泄露,考虑轮换所有 Token

### 场景 3: 检测到暴力破解攻击

**响应步骤**:
1. 确认攻击来源 IP(查看日志中的 `client_ip`)
2. 在防火墙/反向代理层封禁攻击 IP
3. 检查是否有认证成功记录(可能已被攻破)
4. 如果密码强度不足,立即更换为强密码

---

## 合规性考虑

### GDPR (欧盟通用数据保护条例)

- **个人数据**: XiMonitor 不收集最终用户的个人数据,仅记录服务器 IP 地址
- **数据最小化**: 仅存储运维必需的数据(节点 ID, IP, 性能指标)
- **访问控制**: 通过 Basic Auth 限制访问,符合"适当的技术措施"要求
- **数据保留**: 历史数据默认保留 72 小时,可配置

### SOC 2 (服务组织控制)

- **访问控制**: 实现了身份验证(CC6.1)
- **传输加密**: 支持 TLS 加密(CC6.7)
- **日志记录**: 记录认证事件(CC7.2)
- **变更管理**: 通过 Git 版本控制(CC8.1)

### ISO 27001 (信息安全管理)

- **A.9.2.1 用户注册**: 通过 `server issue-node` 注册节点
- **A.9.2.4 密钥管理**: Token 由 CSPRNG 生成,存储权限 0600
- **A.9.4.1 访问限制**: 通过 Basic Auth 和 Token 认证限制访问
- **A.12.4.1 日志记录**: 记录认证失败和会话事件

---

## 联系方式

如发现安全漏洞,请通过以下方式报告:

- **GitHub Security Advisory**: https://github.com/XiNian-dada/NodeLite/security/advisories/new
- **Email**: (待补充)

**请勿公开披露未修复的漏洞**,我们承诺在 90 天内响应并修复。

---

## 变更日志

### 2.0.7 (2026-05-18)
- 更新安全文档，修正项目名称为 NodeLite
- 补充 Token 过期机制说明
- 补充 2FA 安全说明
- 补充密码强度要求

### 1.2.10 (2026-05-12)
- 新增 24 小时会话过期机制(Web UI)
- 新增 `/logout-and-reauth` 端点强制重新登录
- 新增本安全文档

### 1.2.9 (2026-05-12)
- 修复并发 `issue_node` 可能导致的 registry 损坏
- 修复 Mutex poisoning 静默吞掉的问题
- 优化 SQLite 连接复用,减少系统调用

### 1.2.8 及更早
- 实现 HTTP Basic Auth (Web UI)
- 实现 Token-based Auth (Agent)
- 实现 WebSocket 准入控制与速率限制
