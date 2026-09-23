# 2026-09-23 网盘已有文件直链

- Feature commit: `c78be08`, pushed to `origin/feat/directlink-token-api`.
- Frontend deployed to `/opt/baidupcs-rust/web-drive-c78be08` on existing HTTP19529 and HTTPS443 roots. Port80/auth/Token configuration unchanged. Neither backend restarted.
- Nginx configuration backup: `/etc/baidupcs-rust/drive-links-backup-20260923-172943/nginx.conf`. Restore that file to `/www/server/panel/vhost/nginx/baidupcs-rust.conf`, test nginx, reload to roll back. No database rollback needed.
- Existing files use authenticated `GET /api/v1/files/download?fs_id=...&path=...`; no transfer, mkdir or new tokens. Directories default to current-page ordinary-file selection (100 entries/page); explicit confirmation enables recursive sequential extraction. Per-item errors, encrypted-item refusal, stop, account-change cancellation and copy-link/curl supported.
- 35 targeted tests pass; vue-tsc + production build pass. Full suite: 49 pass, 1 fail (existing FilesView search-toggle test). Identical failure reproduced from unmodified HEAD archive. Existing bundle-size warning remains.
- Public index and FilesView asset return200 and exactly match local build. Anonymous original API remains401. Both backend services active.
- Real read-only extraction for already-stored Windows EXE returned HTTP206, bytes0-1023/635948872, MZ header. Root list before/after identical (171 entries), proving this test created no folders.
- Browser visual acceptance not completed: in-app browser rejected Basic login with ERR_INVALID_AUTH_CREDENTIALS. HTTP checks with configured credentials succeeded; mounted desktop/mobile component tests passed.

## Historical empty-folder cleanup (blocked by Baidu credentials)

- Read every page/descendant for exact generated UUID prefix roots; no active transfer tasks.
- Found11 roots:10 entirely empty,1 containing the requested Windows EXE. The nonempty folder was retained.
- Rechecked first empty folder immediately before deletion. Upstream delete rejected errno=-6 (login expired or incomplete credentials/STOKEN). Stopped immediately; zero successful deletions. No user files were removed.
- Private audit on server: `/root/bpr-empty-dir-audit-20260923.json`.
- User must refresh Baidu login (QR login or complete Cookie with STOKEN); re-audit before retry, do not reuse stale deletion eligibility.
- This change does not fix existing share-transfer path creation, auto-clean failed transfer folders, or consolidate share output folders. Those are separate sidecar work. Do not advertise share workflow as repaired.
