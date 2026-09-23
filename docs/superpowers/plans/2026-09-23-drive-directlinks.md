# Existing-drive direct links and historical empty-folder cleanup

> **For agentic workers:** Use superpowers:executing-plans. Follow test-first steps below.

**Goal:** Existing drive files get direct links without transfer or mkdir; folders offer selection and explicitly confirmed recursion. Remove only verified-empty historical extraction folders.

**Architecture:** Vue dialog calls authenticated upstream GET files and files/download (fs_id AND path). Sequential asynchronous iterator handles pagination, bounds, deduplication, cancellation and per-item errors. No new account system, no new transfer tasks. Reuse result dialog with drive-specific notice and curl copy. Keep encrypted items disabled. Existing share workflow remains separate.

**Tech Stack:** Vue3, TypeScript, ElementPlus, Vitest; read-only upstream APIs; narrowly scoped Python maintenance on deployment.

### Task 1 — historical cleanup
- [x] Audit every root matching exact generated UUID prefix, all pages/descendants, exclude active tasks. Record private report on server.
- [ ] Recheck empty immediately before each delete through existing files/delete. Stop on authentication error; no cookie bypass. Verify remaining root entries. Preserve nonempty and unrelated folders.

### Task 2 — direct-link runner/API (TDD)
Files: frontend/src/api/file.ts; frontend/src/utils/driveDirectlink.ts; frontend/src/utils/__tests__/driveDirectlink.test.ts; frontend/src/api/__tests__/fileDirectlink.test.ts.
- [ ] Write/run failing tests: API requires path; one-file success; failure continues; recursive pagination; traversal errors reported; cancellation; encrypted files refused; empty folder; duplicate/outside paths rejected.
- [ ] Update getDownloadUrl(fsId, path). Implement async generator yielding DirectlinkFile records; dependency-injected GET-only API, check cancellation before/after await, sequential requests, path validation, visited sets.
- [ ] Run targeted Vitest suite and vue-tsc.

### Task 3 — UI (TDD)
Files: frontend/src/components/DriveDirectlinkDialog.vue; frontend/src/components/DirectlinkResultDialog.vue; frontend/src/views/FilesView.vue; frontend/src/components/__tests__/DriveDirectlink.test.ts.
- [ ] Write/run failing component tests: auto-extract regular file; folder only lists; ordinary current-directory selection; explicit recursive confirmation; error/copy/progress/stop; no transfer notice for existing files.
- [ ] Add compact desktop/mobile row buttons; dialog for directory paging/navigation, selected ordinary files, confirm recursion, progress and stop; account switch/close cancels stale updates. Show errors individually and successful URLs with copy/curl.
- [ ] Run combined directlink regression tests; full frontend suite and build, report any established upstream failures separately.

### Task 4 — deployment/verification
- [ ] Review diff for credentials and scope; commit and push feature branch per standing deployment request.
- [ ] Upload built frontend into versioned directory; back up nginx config; change only static root references; nginx -t then reload. No backend restart or credential changes.
- [ ] Verify HTTP19529 assets, authenticated read-only directlink for already-stored file and unchanged root folder count. Record deployment and cleanup results. Share-transfer auto-cleanup/grouped folders is not included in this isolated frontend change.

## Execution checkpoint
- Historical audit: 11 generated roots, 10 empty, 1 containing requested EXE. Delete attempt rejected with errno=-6; no successful deletions. Stop until user refreshes Baidu login/STOKEN. Report stored privately on deployment server.
- Implementation complete: GET-only existing-file extraction, 100-entry selection pages, confirmed sequential recursion, cancellation, per-file errors, encrypted-file refusal, desktop/mobile entry points, shared results/curl copy.
- Validation: 35 focused tests pass. Full suite 49 pass/1 fail; same search-toggle failure reproduced from clean HEAD archive (unmodified baseline). Build/typecheck succeeds, existing >500kB bundle warning.
