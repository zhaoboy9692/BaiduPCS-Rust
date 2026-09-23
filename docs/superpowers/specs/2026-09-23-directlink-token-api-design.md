# 独立直链 API 与 Token 管理（最新确认规格）

基线 v2.2.4，定制分支 feat/directlink-token-api。按用户反馈收窄：只获取原 API，沿用原账号、转存和任务管理；不改 uid、不切账号、不新建任务管理系统。

## 边界
- Rust sidecar 仅回环监听，固定回环 upstream；不能让调用者指定内部 URL、Cookie、账号、目标路径。
- `/direct-admin/v1/*` 由 nginx 原 Basic 认证后覆盖注入管理密钥，校验 Origin；浏览器不存 API Token。
- `/direct-api/v1/*` 单独 Bearer Token，不能访问原后台、管理 API。HTTP/HTTPS 双协议按用户要求保留，提示 HTTP 明文风险。
- Token SQLite 只存 SHA256 摘要，创建仅返回一次明文；支持限频、期限、禁用恢复、永久撤销、统计。

## API 和流程
- POST `/shares/preview`：share_url、可选 password，返回有限的安全文件字段，不返回 share_info 内部凭据。
- POST `/resolve`：同上，另可传 selected_fs_ids；同步有限等待，成功返回原 task_id、网盘 save_path、filename/size/url/headers/expires_at:null。
- 管理前缀提供同构接口。不给公共 Token 开放通用 tasks/files 代理。
- 验证百度个人分享 URL → 原预览 → 根目录普通文件筛选 → 生成独立 UUID 目录 → 原普通转存（auto_download=false，is_share_direct_download=false）→ 原任务查询 → 文件列表 → 原下载链接。
- 原项目保持当前账号行为，不扩展账号切换管理。使用者沿用原后台管理账号。
- 默认单并发，最多20个普通文件/2GiB；不递归目录。失败明确提示。同步超时不意味百度转存取消，应到原转存任务页检查；不自动重试转存。
- 文件保留，由原文件管理界面清理；取链接后不立刻删除。不另外记录直链或建设任务/空间预留库。

## 网页与验证
原分享直下保留，新增提取按钮、结果弹框和复制；无需本地下载目录。HTTP 下复制降级并报告真实结果。
专项 Rust/Vue 测试和构建；用本地假上游验证请求契约、错误脱敏及不启动下载。外部可用性必须用授权小文件 Range 实测，不能承诺永久链接/不限速，也不能将账号 Cookie 暴露给调用者。

## 部署
提供独立 nginx/systemd 示例，原 backend 无修改，便于跟上游更新。每次升级跑适配测试。上线前备份前端、配置和数据库；未经验证不替换生产服务。保持原 ws、9701 和 /health 404。
