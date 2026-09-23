# 独立直链服务（开发中：当前仅 Token 管理）

**未实现分享直链、任务编排、占用统计或清理。不得当作完整直链服务部署。**

已实现：独立 SQLite Token 管理、摘要保存、有效期、禁用/恢复、不可恢复撤销、持久化原子限流、管理员通道鉴权、Vue 管理页面、HTTP 复制降级。

## 本地构建与验证

```sh
cargo test --manifest-path directlink-api/Cargo.toml --locked
cargo clippy --manifest-path directlink-api/Cargo.toml --locked --all-targets -- -D warnings
cargo build --manifest-path directlink-api/Cargo.toml --locked --release
cd frontend
npm ci
npm test -- src/views/__tests__/ApiTokensView.test.ts src/utils/__tests__/directlinkClipboard.test.ts
npm run build
```

上游 v2.2.4 全量前端基线已有失败：`FilesView.test.ts` 搜索按钮断言。本模块专项测试独立执行，不能因此称原全量测试通过。

## 配置（不提交真实值）

程序参数是配置文件路径；文件应为 0600，数据库专用目录必须 0700。只允许 Unix 回环监听，拒绝符号链接数据库路径。Rust 程序不记录请求头、Token 和响应体。

```json
{
  "listen": "127.0.0.1:18889",
  "database": "/var/lib/baidupcs-directlink/tokens.sqlite",
  "admin_key": "由 openssl rand -hex 32 生成的独立管理通道密钥",
  "origins": ["http://YOUR_HOST", "https://YOUR_HOST"]
}
```

配置时必须替换示例值，origin 是浏览器实际 scheme+host+port，不带路径或末尾斜线。IPv4/IPv6 IP 和规范域名均可。管理通道密钥不是用户 API Token，不交付调用者。

```sh
./directlink-api/target/release/baidupcs-directlink-api /absolute/private/config.json
```

## 管理 API

仅受信任 Nginx 代理可通过 `X-Directlink-Admin-Key` 管理通道访问。Nginx 必须先验证管理员 Basic 凭据；不要把这个头写进前端代码或交给用户。外部传入的同名头必须覆盖，所有 public location 清空该头；不能直接把 sidecar 暴露到公网。

- `GET /direct-admin/v1/tokens?offset=0&limit=50`，limit 1–100。
- `POST /direct-admin/v1/tokens`，JSON `name`、`note`、`expires_at`（UTC Unix 秒或 null）、`rate_per_minute`（1–60）。返回 201，`data.token` 是唯一明文显示机会。
- `PATCH /direct-admin/v1/tokens/{id}`，可修改上述字段及 `enabled`；省略期限保持原值，显式 null 设为永久。已撤销记录拒绝修改。
- `POST /direct-admin/v1/tokens/{id}/revoke`，永久撤销；重复调用安全。
- 返回 `data` 或 `code`/`message`；所有响应 `Cache-Control: no-store`。没有 `/health` 或凭据找回接口。

网页 `/api-tokens` 沿用现有站点的管理员 Basic 认证。账号权限必须在 Nginx 验证，隐藏菜单不是访问控制。

## 状态和限制

限流为持久化固定分钟窗口，窗口边界存在最多两倍突发（不是滑动窗口）；解析创建和读取分别计数。当前提供已测试的 `Store::admit`，还未连接公共解析路由。统计字段为任务集成预留，目前为 0，不能当成真实提取成功量。

API Token 用 32 字节安全随机数生成，仅存 SHA256 摘要。任意有效 API Token 均不能进入管理接口。HTTP 按用户要求可用，但明文风险无法靠 Token 鉴权消除。

## 下一步兼容性阻塞

上游的 `get_download_url`、文件列表、目录创建和分享预览仍使用 active account。只在请求前后检查账号无法消除并发切换窗口。需要向所需上游接口增加可选显式 uid，使用已有 per-uid client/manager pool；无 uid 时保持旧网页行为。补丁必须覆盖“执行中切换账号仍使用任务绑定账号”和“未知 uid 不回退”。不采用全局切换账号的 workaround。

在此补丁和真实小文件测试前，不启用分享解析 public route，也不替换线上服务。
