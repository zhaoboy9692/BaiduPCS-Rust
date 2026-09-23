# 2026-09-23 独立 Token 直链 API 部署记录

## 部署

- 服务器：用户授权的 42.194.158.165。
- 代码分支：`feat/directlink-token-api`；运行版本 `9525ffa`，前端 `21d3370`（后续仅修复 sidecar）。
- CI：https://github.com/zhaoboy9692/BaiduPCS-Rust/actions/runs/35816538692
- 原版 v2.2.4 后端仍运行于 `127.0.0.1:18888`，未替换其账号、转存、任务管理。
- 新增 `baidupcs-directlink.service`，独立低权限用户，监听 `127.0.0.1:18889`，已设置开机启动。
- 二进制 `/opt/baidupcs-directlink/current`，SQLite `/var/lib/baidupcs-directlink/tokens.sqlite`，私有配置 `/etc/baidupcs-directlink/config.json`。
- HTTP、HTTPS 均提供新前端及接口。仅 `/direct-api/` 免除网站 Basic，仍要求独立 Bearer Token。`/direct-admin/` 保留原网站认证和源站检查。
- 原网站管理员密码未在此次部署更改；不在记录中保存密码或 Token。
- MobileAndProxy 的 9701 配置及 HTTP 旧 `/ws` 转发保持不变。
- 回滚备份：`/etc/baidupcs-rust/directlink-backup-20260923-115551`。恢复 nginx 配置后先配置校验再重载；sidecar 可独立停止。

## 验证结果

- Rust 18 个测试通过，fmt/clippy 通过。
- 前端本次相关 11 个测试、类型检查、构建通过。上游全量前端测试存在已知 FilesView 搜索断言失败，不声称全量通过。
- 双协议认证后 Token 页面、文件页面、管理员 Token API、原账号 API 返回 200。
- 临时 Token 创建 201，真实分享预览 200；响应不暴露 share_info/bdstoken。
- 匿名或伪造管理员头不能访问管理接口；恶意 Origin 403；禁用 Token 403；撤销 Token 401。
- 测试 Token 已撤销，审计记录保留。`/health` 返回 404。
- 前端新资源已由服务器提供；浏览器工具因 Basic 认证失败，未完成视觉验收。
- 原版取直链接口取得自建小文件链接；服务端及外部 Mac 不带 Cookie、使用指定 UA 的 Range 下载均返回 206，内容一致。

## 尚未通过的验收及残留

不能据上述独立检查宣称“分享 → 转存 → 直链”完整链路已通过。

- 当前账号自己的测试分享转回本账号，原转存接口报 `errno=2 / 文件已存在`。绕过 sidecar 直接调用原接口同样失败。
- 测试中曾遇到原接口未获取 bdstoken；通过已有 Cookie 刷新同一账号后创建目录成功，未加入账号切换逻辑。
- 最后删除测试文件时原接口报 `errno=-6`，要求重新扫码登录或导入含 STOKEN 的完整 Cookie。因此还需恢复账号有效登录并提供其他账号分享的小文件完成验收。
- 测试分享已取消；生成的转存任务已删除；下载任务数为 0。
- 删除未成功，网盘保留以下无敏感数据的测试内容，登录恢复后可在原文件页删除：
  - `/directlink-smoke-cb62b3b2c7.txt`（60 字节测试文本）
  - `/.bpr_directlink_api_daeef406-2008-4177-81b3-40072b68dbe2`（空目录）
  - `/.bpr_directlink_api_7a4b382e-87f6-4a09-98cf-c1ded3a74a47`（空目录）

HTTP 按用户要求保留，但密码/Token 通过 HTTP 传输不加密；支持 HTTP 不等于安全性与 HTTPS 相同。
