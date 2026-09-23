# 独立直链 API

在原 BaiduPCS-Rust 上增加独立 API Token。账号、转存和任务管理沿用原项目；原 backend 无改动。新增 Vue「API Token」页面与分享直下中的「提取直链」按钮、链接弹框、复制链接/请求头。

## 构建与测试

```sh
cargo fmt --manifest-path directlink-api/Cargo.toml --check
cargo test --manifest-path directlink-api/Cargo.toml --locked
cargo clippy --manifest-path directlink-api/Cargo.toml --locked --all-targets -- -D warnings
npm test --prefix frontend -- src/views/__tests__/ApiTokensView.test.ts src/utils/__tests__/directlinkClipboard.test.ts src/components/__tests__/ShareDirectlink.test.ts src/components/__tests__/ShareFileExtractionSelection.test.ts
npm run build --prefix frontend
```

上游全量前端基线存在 FilesView 搜索按钮断言失败，专项通过不代表全量通过。Actions 构建 Linux amd64 musl 静态 sidecar，兼容旧 glibc 主机。

## 配置

配置0600、数据库专用目录0700；Unix 回环监听，拒绝符号链接数据库路径。示例见 `deploy/config.example.json`，必须替换管理通道密钥与 origin。`upstream_url` 固定到原服务回环 HTTP，禁止任意主机/路径；可选 `upstream_bearer` 仅用于原服务内部认证，不是百度 Cookie，不返回调用者。省略 upstream_url 时仅 Token 管理可用。

启动参数：`baidupcs-directlink-api /absolute/private/config.json`。

## 网页提取范围

输入链接后点击「选择分享/提取文件」，只在文件选择页显示「提取直链」。提取当前目录已加载列表中勾选的普通文件，空选择禁用；文件夹请先进入再选文件，不自动递归。「全选」只操作当前列表普通文件；「下载全选（含文件夹）」保留原下载能力。目录勾选、跨目录下载选项不作为网页直链请求。每个成功项有独立复制按钮，失败项显示原因。原转存/同步选择器不启用此界面模式。下面的 API 递归规则不受网页限制影响。

## 公共 API

`Authorization: Bearer dl_...`。不支持查询串 Token、Cookie 或 Basic 替代。Token 不能访问后台或管理接口。

```sh
curl --fail-with-body 'https://YOUR_HOST/direct-api/v1/resolve' \
  -H "Authorization: Bearer $DIRECTLINK_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"share_url":"https://pan.baidu.com/s/SHARE_ID","password":"abcd"}'
```

- POST `/direct-api/v1/shares/preview`：递归分页浏览，返回 `data.files` 普通文件列表（fs_id/name/path/is_dir/size），不返回 share_info 内部凭据。浏览不计累计提取次数。
- POST `/direct-api/v1/resolve`：省略 `selected_fs_ids` 时自动遍历所有文件夹中的普通文件。可传文件/文件夹 ID，选中文件夹递归展开；可选 `selected_paths` 仅作目录剪枝提示，元数据必须由原接口重新核实。无需在客户端传百度凭据。
- 不限制文件个数或文件大小。每文件一个原转存任务/独立 UUID 目录；同名文件也不相互覆盖。一个失败不阻断后续文件。返回顶层 JSON `list`（不再是旧版 `data.files`）：

```json
{
  "list": [
    {"fs_id":123,"name":"example.bin","path":"/folder/example.bin","is_dir":false,"size":5368709120,"success":true,"url":"https://download.example/file","headers":{"User-Agent":"原下载器UA"},"expires_at":null,"task_id":"原转存任务ID","save_path":"/.bpr_directlink_api_uuid","error":null},
    {"fs_id":124,"name":"failed.bin","path":"/folder/failed.bin","is_dir":false,"size":100,"success":false,"url":null,"headers":{},"expires_at":null,"task_id":null,"save_path":null,"error":{"code":"token_quota_exhausted","message":"Token 提取次数已用尽，请联系管理员增加额度"}}
  ],
  "total":2,"succeeded":1,"failed":1,"complete":true
}
```

`size` 单位字节。`complete` 表示目录是否完整枚举，不表示所有文件成功。读取目录失败时有 `is_dir:true` 失败条目、`complete:false`；顶层严重错误可返回 `error:{code,message}`。调用方必须检查每项 `success/error` 和顶层 `complete/error`，不能仅看 HTTP 200。

- 长批次采用 HTTP chunked 输出**一个完整 JSON 文档**，不是 NDJSON。逐文件写出记录，10秒空白心跳避免 nginx 空闲超时；使用 `response.json()` 需等完整响应，调用端不要设190秒整批超时。响应头禁用 nginx 缓冲；反代空闲超时仍195秒。认证/输入错误在流开始前返回正常4xx；流开始后的错误写入 JSON。
- 每文件最多等待180秒，单次原接口30秒超时；没有全批180秒上限。客户端断开停止后续文件，但已启动的原任务不能保证取消，应查看原转存任务页。没有新增第二套任务管理。
- 原普通转存显式 `auto_download=false`、`is_share_direct_download=false`，不下载文件内容到服务器。目录 `/.bpr_directlink_api_<uuid>` 保留，用原文件管理页清理；删除可能使直链失效。
- HTTP/HTTPS均可。HTTP传输Token和链接为明文，公网推荐HTTPS。返回原下载器 User-Agent，不返回百度 Cookie，不承诺永久有效或免限速。

错误包含 invalid_token、token_disabled、token_expired、token_quota_exhausted、rate_limited、selection_not_found、directory_failed、upstream_unavailable、resolve_timeout、transfer_failed。错误不透传原始凭据。

## 管理 API

网页 `/api-tokens` 使用原站管理员 Basic；服务端必须经 nginx 认证后覆盖注入 `X-Directlink-Admin-Key`，浏览器不可保存/提交这个密钥。管理同构 `/direct-admin/v1/resolve`、`/shares/preview`；无 API Token 也能在网页提取。

- GET `/direct-admin/v1/tokens?offset=0&limit=50`（limit1–100）
- POST `/direct-admin/v1/tokens`：name、note、expires_at（UTC秒/null）、rate_per_minute（1–60）、max_uses（累计文件尝试上限，null不限，0不允许提取）。201响应 `data.token` **仅返回一次明文**。
- PATCH `/direct-admin/v1/tokens/{id}`：修改信息/期限/限流/enabled；省略期限不改，null永久。max_uses同样省略保留、null不限，可增加上限。
- POST `/direct-admin/v1/tokens/{id}/revoke`：永久撤销当前密钥，普通“恢复”不会复活它。
- POST `/direct-admin/v1/tokens/{id}/rotate`：重置 Token，生成全新密钥并启用，旧密钥永久失效；保留过期时间、次数上限、已用次数和历史统计。返回 `data.token` 仅显示一次。过期的记录仍需编辑有效期。
- POST `/direct-admin/v1/tokens/{id}/reset-usage`：清零 `used_count`，保留上限、成功/失败统计和密钥状态。正在运行的提取会继续计数。

累计次数按**每文件实际尝试**计1次（成功或失败均计；预览、没找到文件和额度不足未执行不计）。在 SQLite IMMEDIATE 事务中预约，防并发超额；每文件重新校验密钥/启用状态/期限，撤销或轮换后旧请求不能继续预约。返回 `max_uses` 与 `used_count`，剩余为上限减已用。旧数据库自动增加字段，旧 Token 默认不限，新增累计计数从升级后开始，不把历史按请求统计冒充文件次数。

SQLite只存随机256bit Token摘要。期限精确至秒，界面显示日期时分；撤销、禁用、到期每次请求校验。解析默认10次/分钟，预览60次/分钟，持久化固定窗口，窗口边界有突发。单并发提取/预览；新提取按每文件已预约尝试记录成功/失败；旧版本遗留统计按请求计算。断开或进程被杀可能已扣次数但未记录最终结果，不声称精确计费。

## 部署边界

见 deploy。不要替换整个现有 nginx 配置；将两个 location 加入既有 HTTP/HTTPS server，保留原 ws、9701。管理继承 Basic，public 仅窄路径取消 Basic，仍强制 Bearer；清空 public 管理头，管理头服务端覆盖。两服务仅回环，/health仍404。日志不记录请求/Token/直链。

上线先备份前端、nginx、配置/SQLite；上传版本目录，用 symlink 切换前端；`nginx -t` 后 reload。sidecar独立 systemd，不重启原 backend。静态首页 no-store，避免旧按钮缓存。回退原nginx配置及前端路径，停止sidecar即可。
