# 独立直链 API

在原 BaiduPCS-Rust 上增加独立 API Token。账号、转存和任务管理沿用原项目；原 backend 无改动。新增 Vue「API Token」页面与分享直下中的「提取直链」按钮、链接弹框、复制链接/请求头。

## 构建与测试

```sh
cargo fmt --manifest-path directlink-api/Cargo.toml --check
cargo test --manifest-path directlink-api/Cargo.toml --locked
cargo clippy --manifest-path directlink-api/Cargo.toml --locked --all-targets -- -D warnings
npm test --prefix frontend -- src/views/__tests__/ApiTokensView.test.ts src/utils/__tests__/directlinkClipboard.test.ts src/components/__tests__/ShareDirectlink.test.ts
npm run build --prefix frontend
```

上游全量前端基线存在 FilesView 搜索按钮断言失败，专项通过不代表全量通过。Actions 构建 Linux amd64 musl 静态 sidecar，兼容旧 glibc 主机。

## 配置

配置0600、数据库专用目录0700；Unix 回环监听，拒绝符号链接数据库路径。示例见 `deploy/config.example.json`，必须替换管理通道密钥与 origin。`upstream_url` 固定到原服务回环 HTTP，禁止任意主机/路径；可选 `upstream_bearer` 仅用于原服务内部认证，不是百度 Cookie，不返回调用者。省略 upstream_url 时仅 Token 管理可用。

启动参数：`baidupcs-directlink-api /absolute/private/config.json`。

## 公共 API

`Authorization: Bearer dl_...`。不支持查询串 Token、Cookie 或 Basic 替代。Token 不能访问后台或管理接口。

```sh
curl --fail-with-body 'https://YOUR_HOST/direct-api/v1/resolve' \
  -H "Authorization: Bearer $DIRECTLINK_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"share_url":"https://pan.baidu.com/s/SHARE_ID","password":"abcd"}'
```

- POST `/direct-api/v1/shares/preview`：同上输入，返回 `data.files`（fs_id/name/path/is_dir/size）。仅预览第一页最多21项，不返回 share_info 内部凭据。
- POST `/direct-api/v1/resolve`：同上，可选 `selected_fs_ids:[123,456]`（必须来自预览）。成功200：`data.task_id` 是**原任务ID**，`save_path` 是网盘保留目录，`files:[{filename,size,url,headers,expires_at:null}]`。
- HTTP/HTTPS均可。HTTP传输Token和链接为明文，公网推荐HTTPS。
- 同步最长180秒，调用端/反代超时需大于190秒。不提供第二套 jobs 查询/管理接口。超时、断开或重启不能保证百度侧转存取消，应查看原转存任务页，不立即重复提交。
- 原普通转存显式 `auto_download=false`、`is_share_direct_download=false`，不下载内容到服务器。根目录 UUID 专用目录 `/.bpr_directlink_api_<uuid>` 保留，用原文件管理页清理；删除可能使直链失效。
- 第一版仅支持根目录普通文件，每次最多20个、总计2GiB，不递归目录。预览/选择不在第一页、目录、超量请求明确报错。限流是请求数，不是下载带宽/网盘累计空间配额。
- 链接自带权限，勿泄漏。返回 upstream 下载器使用的 User-Agent，不返回百度 Cookie，不承诺永久有效、免限速；外部可用性需真实小分享实测。

错误为 `{code,message}`：401 invalid_token；403 token_disabled/token_expired；429 rate_limited（Retry-After）；400 invalid_input/share_password_required/share_password_invalid/share_unavailable；422 file_limit；503 resolver_busy/upstream_unavailable；504 resolve_timeout；502 transfer_failed。错误不透传上游原始内容。

## 管理 API

网页 `/api-tokens` 使用原站管理员 Basic；服务端必须经 nginx 认证后覆盖注入 `X-Directlink-Admin-Key`，浏览器不可保存/提交这个密钥。管理同构 `/direct-admin/v1/resolve`、`/shares/preview`；无 API Token 也能在网页提取。

- GET `/direct-admin/v1/tokens?offset=0&limit=50`（limit1–100）
- POST `/direct-admin/v1/tokens`：name、note、expires_at（UTC秒/null）、rate_per_minute（1–60）。201响应 `data.token` **仅返回一次明文**。
- PATCH `/direct-admin/v1/tokens/{id}`：修改信息/期限/限流/enabled；省略期限不改，null永久。
- POST `/direct-admin/v1/tokens/{id}/revoke`：永久撤销，不可恢复。

SQLite只存随机256bit Token摘要。期限精确至秒，界面显示日期时分；撤销、禁用、到期每次请求校验。解析默认10次/分钟，预览60次/分钟，持久化固定窗口，窗口边界有突发。单并发提取/预览；有效解析进入上游执行后按请求结局记成功/失败，鉴权失败/输入非法/忙碌不算结果，断开后已开始的有限工作继续并计数；进程被杀可能不计数，不声称精确计费。

## 部署边界

见 deploy。不要替换整个现有 nginx 配置；将两个 location 加入既有 HTTP/HTTPS server，保留原 ws、9701。管理继承 Basic，public 仅窄路径取消 Basic，仍强制 Bearer；清空 public 管理头，管理头服务端覆盖。两服务仅回环，/health仍404。日志不记录请求/Token/直链。

上线先备份前端、nginx、配置/SQLite；上传版本目录，用 symlink 切换前端；`nginx -t` 后 reload。sidecar独立 systemd，不重启原 backend。静态首页 no-store，避免旧按钮缓存。回退原nginx配置及前端路径，停止sidecar即可。
