# 2026-09-23 网页选择提取与19529端口部署

- 用户确认公网端口19529，不再使用80端口提供网盘服务。
- 代码 `ef91b2751a80e9c67aad48b23e0a8c8911337fd7` 已推送 feature 分支。
- CI成功：https://github.com/zhaoboy9692/BaiduPCS-Rust/actions/runs/35824729362 。本地20项前端专项测试、30项Rust测试、类型检查/构建通过，既有chunk大小警告保留。
- 前端 `/opt/baidupcs-rust/web-directlink-ef91b27`，修改输入/选择页提取入口及普通文件选择范围，原转存/同步选择器不变。
- 新入口 `http://42.194.158.165:19529/`，管理页 `/api-tokens`，API `/direct-api/v1/resolve`。
- nginx新端口使用原站Basic和原认证边界，新增精确Origin allowlist；sidecar同步允许新Origin。
- 原80端口网盘/API入口均移除、返回404，仅保留不相关的原 `/ws` → 10500 转发。没有关闭整台服务器的80端口。
- 原HTTPS443入口保留并更新前端；19529为HTTP，不支持将该URL直接改成HTTPS。HTTP明文传输凭据，风险不因改端口消失。
- sidecar二进制仍50b8428，仅为读取Origin配置重启。数据库/Token未修改；原18888后端未重启；18888/18889仍仅回环。
- 备份 `/etc/baidupcs-rust/port19529-backup-20260923-140201`，包含原nginx和私有sidecar配置，权限限制。回滚恢复这两个文件、nginx检查/reload、sidecar restart即可；不应覆盖Token数据库。

## 公网实测

- 新端口首页、Token页面、带新Origin认证后的管理员接口200。
- 匿名管理员/直链API401，恶意Origin管理员请求403，health404。
- 80端口首页、Token页、direct-admin及direct-api均404。
- 新端口index.html及ShareDirectDownloadDialog-CEMjRxv-.js与本地构建逐字节一致。
- 9701登录页200。两网盘服务active。
- 本轮无真实网盘转存写操作，未声称修复历史原转存“路径不存在”问题，也未完成浏览器视觉验收。
