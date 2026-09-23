# Share directlink transfer path fix

## Evidence and scope

The deployed v2.2.4 transfer task failed with `转存路径不存在` (errno 2).
Share preview uses virtual `/sharelink<uk>-<shareid>/...` paths while the
share title may be in the owner's private path namespace. The original
transfer manager strips the virtual root, groups by parent and preserves
nested destination directories. Its directory existence probe may accept
an empty successful listing for a missing directory, skipping creation.
The adapter also assumed the result would be directly in its UUID folder.

The adapter transfers exactly one ordinary file per task, identified by its
trusted, preview-validated `fs_id`. Pass a cloned selected-file record with
`path=/<validated name>` as destination grouping metadata. Preserve the real
source path in public results. This keeps destination grouping at root for
both private and virtual share paths, avoids unnecessary child folders, and
retains strict saved path/name/size validation before obtaining a link.
Account/task handling stays with the original upstream. Existing-drive directlinks,
web selection behavior, and API recursive discovery are unchanged.

Task failures classify only allowlisted messages for missing destination and
expired/incomplete login. Raw error text, cookies and signed URLs are not exposed.

## Verification

- Regression fake now models v2.2.4 destination grouping and missing child dirs.
- Before fix: four tests failed, including the nested/deep extraction scenarios.
- After fix: 32 sidecar tests pass; cargo clippy --locked --all-targets -- -D warnings passes.
- Coverage includes flat/private/virtual paths, original source preservation,
  recursive discovery, quota accounting, mixed outcomes and sanitized errors.

## Deployment checklist

Build the Linux musl artifact through Directlink API (development) CI, preserve
config and tokens.sqlite, keep previous binary for rollback, switch sidecar only,
then submit selected file through the normal authenticated resolve endpoint.
Verify a small Range download without executing content. Do not count a manually
created directory or a previously saved file as proof of automatic share extraction.

Historical empty-directory cleanup is separate: the last delete attempt was
blocked by expired/incomplete upstream login credentials. No deletion is part of
this fix; successful share staging files must remain while their links are in use.
