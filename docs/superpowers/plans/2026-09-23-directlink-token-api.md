# Directlink Token API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver an independently authenticated direct-link API and token administration in the existing Vue app without server downloads.

**Architecture:** Rust Axum sidecar, SQLite, loopback-only listener; nginx authenticates administrator channel and strips spoofed headers. Public Bearer credentials never grant admin access. Upstream adapter must use explicit account binding rather than changing active account.

**Tech Stack:** Rust, Axum 0.7, rusqlite bundled, Tokio, Vue 3, Element Plus, Vitest.

## Execution order and checkpoints

### 1. Baseline
- [ ] `cd frontend && npm ci && npm test && npm run build`; record pre-existing failures, never silently modify them to turn results green.
- [ ] Keep the fresh clone's existing feature branch; no extra worktree needed to protect other work.

### 2. Token persistence and authorization (first independently testable component)
Files: `directlink-api/Cargo.toml`, `directlink-api/src/{lib,store,error,http,main}.rs`, `directlink-api/tests/{store,http}.rs`.
- [ ] Write tests first: create a token, reopen DB, authorize same value, list never serializes secret/hash; revoke cannot be restored; expiration at exact timestamp rejects.
- [ ] Run `cargo test --manifest-path directlink-api/Cargo.toml`, observe missing implementation failure, then implement CSPRNG token + SHA256 digest + parameterized SQLite transactions.
- [ ] Test public Bearer rejected on admin route; empty config rejected; origin mismatch rejected; malformed inputs do not cause 500.
- [ ] Implement admin CRUD routes, body size limits, JSON error contract, fixed-window durable rate counters with immediate transactions. No placeholder successful extraction response.
- [ ] Test two SQLite connections competing for quota, pagination, bad ids, permanent revoke, expiry cannot be cleared accidentally by an omitted edit field.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.

### 3. Token administration UI
Files: `frontend/src/api/directlink.ts`, `frontend/src/utils/directlinkClipboard.ts`, `frontend/src/views/ApiTokensView.vue`, corresponding `*.test.ts`; existing `layouts/MainLayout.vue`, `router/index.ts`.
- [ ] Test copy secure API success, HTTP fallback and failure first, then implement.
- [ ] Test list/create/revoke errors via mounted component and mocked HTTP boundary before implementation.
- [ ] Implement separate axios client without upstream interceptors. Add desktop/mobile entry after settings, route /api-tokens.
- [ ] Create form: name/note, optional datetime, rate 1–60. Full token only in one-time modal; reset secret on close. Tight table, edit, disabled/expired/revoked status, confirmation on revoke.
- [ ] Tests ensure expired is not permanent and revoked cannot be enabled; browser errors never claim success.

### 4. Account-bound upstream adapter and jobs
Files: `directlink-api/src/{upstream,jobs,limits}.rs`, `directlink-api/tests/jobs.rs`, small upstream compatibility changes only if required.
- [ ] Verify explicit UID routing for preview/list/locate; never use global active account mutation. Review necessary compatibility patch before proceeding if upstream cannot provide this.
- [ ] Local fake HTTP upstream verifies preview -> save UUID directory -> normal transfer with auto_download=false -> poll -> locate; no download task calls.
- [ ] Durable jobs, ownership, atomic space reservations, max 20 files/2 GiB per task/10 GiB retained, global concurrency 1. Strict Baidu URL and file validation, no arbitrary target paths or host requests.
- [ ] Error/timeout/restart states retain reservations until reconciliation; no destructive guessing. Only registered terminal job directories may be manually cleaned.
- [ ] API response includes required headers but never a Baidu cookie; if links need cookie, report incompatibility rather than leak credentials.

### 5. Share UI, packaging and integration
Files: `frontend/src/components/ShareDirectDownloadDialog.vue`, `DirectlinkResultDialog.vue`, sidecar docs/config/systemd/nginx and `.github/workflows/directlink-api.yml`.
- [ ] TDD selection preservation, loading, polling cancel, error display, copy result.
- [ ] Add optional env-driven sidecar availability, no long-lived token in frontend storage.
- [ ] CI builds sidecar and tests frontend; include config without real credentials. HTTP/HTTPS dual nginx example preserves existing ws and auth boundaries.
- [ ] End-to-end limited Range download needs an authorized sample share and logged-in account. Do not claim complete before this verification. Deploy only after backups and local tests, never silently replace running backend.

## Completion evidence
Record exact test totals/build results and unmet criteria. Do not enable incomplete public resolver or label token management as full directlink implementation.

## 2026-09-23 checkpoint

Implemented token store, HTTP administration, secure runtime configuration, Vue admin page, desktop/mobile navigation, clipboard fallback, targeted CI. Store/HTTP/runtime tests: 12 passing. Frontend added tests: 9 passing. Real loopback process smoke: anonymous 401, create 201, redacted list, revoke 200, revoked restore 403. Full frontend baseline had one pre-existing FilesView search failure; build passed. No production changes.

Account-bound extraction remains blocked on a small upstream compatibility patch: optional explicit uid for preview, file list, mkdir and locate; with old behavior retained when omitted. Current active-account-only APIs cannot guarantee task account isolation. Public resolve route, share button, cleanup and deployment are deliberately not enabled. Before implementing the compatibility patch, review this deviation from the preferred sidecar-only approach.
