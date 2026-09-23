# Nested share per-file extraction implementation plan

**Goal:** 修复子目录所选文件找不到的问题；按用户确认取消20文件/2GiB业务限制，逐文件提取与部分失败展示。
**Architecture:** 原服务仍负责账号/目录浏览/转存/直链。sidecar重新取得分享凭据，仅内部使用，分页遍历可信目录核实所选ID；前端附带文件路径帮助剪枝（不信任其大小或凭据）。每个文件独立原转存任务/目录。UI逐文件请求，展示进度和部分失败，不隐式下载内容。
**Tech Stack:** Rust/axum/reqwest + Vue3/TypeScript/Element Plus。

- [ ] Rust回归：嵌套普通文件、超过20个文件、超过2GiB、分页、伪造ID、子目录凭据不泄漏；先 `cargo test --manifest-path directlink-api/Cargo.toml --test upstream` 看失败。
- [ ] 替换根目录20条过滤为分页可信目录遍历；可选path hints只剪枝；分开folder/selection错误；文件独立转存与超时，结果包含每文件task/path或error。
- [ ] Vue回归：选中的子目录path传入；21个大文件逐个调用、单失败继续、结果/进度可见。先 `npm test -- src/components/__tests__/ShareDirectlink.test.ts` 看失败。
- [ ] 前端批量编排复用原选文件UI；无选择时先进入选择而非自动整分享转存；结果保留成功链接和失败项。
- [ ] Rust全量+fmt+clippy，Vue相关测试+构建；更新API文档。git diff检查、提交推送。
- [ ] CI生成musl二进制后备份并更新42的sidecar/前端，nginx校验和重载；使用授权分享测试选中子目录文件，只Range取少量数据，不下载整个安装包；报告实际结果。

技术保护仍保留：独立Token/限流、单提取并发、每页100条、目录响应1MiB、发现阶段180秒、单文件180秒、请求体上限。它们不是按文件大小或选择数量拒绝；大批量调用方逐文件请求，不把整个批次绑在单个HTTP超时上。用户追加确认：不指定文件时递归所有文件夹；选中文件夹也递归其普通文件。API顶层list，逐文件成功/失败记录；目录枚举失败单独记录，complete=false不隐瞒遗漏。批次无总超时，HTTP分块返回JSON并用空白心跳避免反代空闲超时；断开停止后续文件，已开始原任务不保证取消。


追加：SQLite `max_uses`/`used_count`，每文件预约次数，预览免费，失败计数，旧表自动迁移；用户确认后的「重置Token」轮换新密钥不恢复旧密钥，「重置次数」只清零累计次数，均确认操作并保留历史统计。测试覆盖迁移、跨连接原子预约、部分失败/超额、轮换使在途旧凭据失效。
