use baidupcs_directlink_api::store::{CreateToken, EditToken, Store};

fn input() -> CreateToken {
    CreateToken {
        name: "脚本".into(),
        note: "测试".into(),
        expires_at: Some(200),
        rate_per_minute: 2,
        max_uses: None,
    }
}

#[test]
fn persists_digest_not_secret_and_honors_exact_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.sqlite");
    let store = Store::open(&path).unwrap();
    let issued = store.create(input(), 100).unwrap();
    assert!(issued.token.starts_with("dl_"));
    assert!(issued.token.len() >= 46);
    assert_eq!(
        store.authorize(&issued.token, 199).unwrap().id,
        issued.record.id
    );
    assert_eq!(
        store.authorize(&issued.token, 200).unwrap_err().code(),
        "token_expired"
    );
    let serialized = serde_json::to_string(&store.list(0, 50).unwrap()).unwrap();
    assert!(!serialized.contains(&issued.token));
    assert!(!serialized.contains("secret_hash"));
    drop(store);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let hash: String = conn
        .query_row("SELECT secret_hash FROM api_tokens", [], |r| r.get(0))
        .unwrap();
    assert_ne!(hash, issued.token);
    assert_eq!(hash.len(), 64);
    let reopened = Store::open(&path).unwrap();
    assert!(reopened.authorize(&issued.token, 199).is_ok());
    assert_eq!(
        reopened.authorize("bogus", 150).unwrap_err().code(),
        "invalid_token"
    );
}

#[test]
fn disable_restore_and_permanent_revoke() {
    let store = Store::open_memory().unwrap();
    let issued = store.create(input(), 100).unwrap();
    store
        .edit(
            &issued.record.id,
            EditToken {
                enabled: Some(false),
                ..Default::default()
            },
            110,
        )
        .unwrap();
    assert_eq!(
        store.authorize(&issued.token, 111).unwrap_err().code(),
        "token_disabled"
    );
    store
        .edit(
            &issued.record.id,
            EditToken {
                enabled: Some(true),
                ..Default::default()
            },
            112,
        )
        .unwrap();
    assert!(store.authorize(&issued.token, 113).is_ok());
    store.revoke(&issued.record.id, 114).unwrap();
    assert_eq!(
        store.authorize(&issued.token, 115).unwrap_err().code(),
        "invalid_token"
    );
    assert_eq!(
        store
            .edit(
                &issued.record.id,
                EditToken {
                    enabled: Some(true),
                    ..Default::default()
                },
                116
            )
            .unwrap_err()
            .code(),
        "token_revoked"
    );
}

#[test]
fn editing_note_preserves_expiry_and_explicit_null_clears_it() {
    let store = Store::open_memory().unwrap();
    let issued = store.create(input(), 100).unwrap();
    let edit: EditToken = serde_json::from_str(r#"{"note":"new"}"#).unwrap();
    let updated = store.edit(&issued.record.id, edit, 110).unwrap();
    assert_eq!(updated.expires_at, Some(200));
    let edit: EditToken = serde_json::from_str(r#"{"expires_at":null}"#).unwrap();
    let updated = store.edit(&issued.record.id, edit, 111).unwrap();
    assert_eq!(updated.expires_at, None);
    assert!(store.authorize(&issued.token, 99999).is_ok());
}

#[test]
fn durable_rate_limits_and_separate_read_bucket() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let store = Store::open(&path).unwrap();
    let issued = store.create(input(), 100).unwrap();
    store.admit(&issued.token, 121, false).unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    store.admit(&issued.token, 122, false).unwrap();
    assert_eq!(
        store.admit(&issued.token, 123, false).unwrap_err().code(),
        "rate_limited"
    );
    assert!(store.admit(&issued.token, 124, true).is_ok());
    assert!(store.admit(&issued.token, 180, false).is_ok());
}

#[test]
fn quota_admission_is_atomic_across_connections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    let store = Store::open(&path).unwrap();
    let mut create = input();
    create.rate_per_minute = 1;
    let issued = store.create(create, 100).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let s = Store::open(&path).unwrap();
            let b = barrier.clone();
            let t = issued.token.clone();
            std::thread::spawn(move || {
                b.wait();
                s.admit(&t, 121, false).is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|v| *v)
            .count(),
        1
    );
}

#[test]
fn rejects_invalid_input_and_bounds_pagination() {
    let store = Store::open_memory().unwrap();
    let mut create = input();
    create.rate_per_minute = 0;
    assert_eq!(
        store.create(create, 100).unwrap_err().code(),
        "invalid_input"
    );
    let mut create = input();
    create.expires_at = Some(100);
    assert!(store.create(create, 100).is_err());
    let mut create = input();
    create.name = "  ".into();
    assert!(store.create(create, 100).is_err());
    assert!(store.list(0, 1000).is_err());
    assert_eq!(
        store.revoke("missing", 100).unwrap_err().code(),
        "not_found"
    );
}

#[test]
fn resolve_result_counters_are_durable_and_independent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tokens.sqlite");
    let store = Store::open(&path).unwrap();
    let issued = store
        .create(
            serde_json::from_value(serde_json::json!({"name":"stats"})).unwrap(),
            1,
        )
        .unwrap();
    store.record_result(&issued.record.id, true).unwrap();
    store.record_result(&issued.record.id, false).unwrap();
    drop(store);
    let record = Store::open(&path)
        .unwrap()
        .authorize(&issued.token, 2)
        .unwrap();
    assert_eq!((record.success_count, record.failure_count), (1, 1));
}

#[test]
fn lifetime_file_quota_is_persistent_editable_and_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quota.db");
    let store = Store::open(&path).unwrap();
    let issued = store
        .create(
            serde_json::from_str(r#"{"name":"quota","max_uses":1}"#).unwrap(),
            100,
        )
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let s = Store::open(&path).unwrap();
            let id = issued.record.id.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                s.reserve_file(&id, 101).is_ok()
            })
        })
        .collect();
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|h| h.join().ok())
            .filter(|x| *x)
            .count(),
        1
    );
    assert_eq!(
        store
            .reserve_file(&issued.record.id, 102)
            .unwrap_err()
            .code(),
        "token_quota_exhausted"
    );
    assert!(store.admit(&issued.token, 102, true).is_ok());
    let r = store
        .edit(
            &issued.record.id,
            serde_json::from_str(r#"{"note":"keep"}"#).unwrap(),
            103,
        )
        .unwrap();
    assert_eq!(r.max_uses, Some(1));
    assert_eq!(r.used_count, 1);
    store
        .edit(
            &issued.record.id,
            serde_json::from_str(r#"{"max_uses":null}"#).unwrap(),
            104,
        )
        .unwrap();
    store.reserve_file(&issued.record.id, 105).unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    let r = store.list(0, 50).unwrap().remove(0);
    assert_eq!(r.used_count, 2);
    assert_eq!(r.max_uses, None);
    store.revoke(&issued.record.id, 106).unwrap();
    assert!(store.reserve_file(&issued.record.id, 107).is_err());
}

#[test]
fn reset_usage_preserves_audit_and_rotation_never_restores_old_secret() {
    let store = Store::open_memory().unwrap();
    let issued = store.create(input(), 100).unwrap();
    store.reserve_file(&issued.record.id, 101).unwrap();
    store.record_result(&issued.record.id, false).unwrap();
    store.revoke(&issued.record.id, 102).unwrap();
    let rotated = store.rotate(&issued.record.id, 103).unwrap();
    assert_ne!(issued.token, rotated.token);
    assert!(store.authorize(&issued.token, 104).is_err());
    assert!(store.authorize(&rotated.token, 104).is_ok());
    assert_eq!(rotated.record.used_count, 1);
    let reset = store.reset_usage(&issued.record.id, 105).unwrap();
    assert_eq!(reset.used_count, 0);
    assert_eq!(reset.failure_count, 1);
    assert_eq!(reset.expires_at, Some(200));
}

#[test]
fn rotation_invalidates_reservations_by_old_authenticated_caller() {
    let store = Store::open_memory().unwrap();
    let issued = store.create(input(), 100).unwrap();
    store.rotate(&issued.record.id, 101).unwrap();
    assert!(store
        .reserve_authenticated(&issued.record.id, &issued.token, 102)
        .is_err());
    assert_eq!(store.list(0, 50).unwrap()[0].used_count, 0);
}

#[test]
fn migrates_existing_deployed_database_without_changing_token_or_audit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    let store = Store::open(&path).unwrap();
    let issued = store.create(input(), 100).unwrap();
    store.record_result(&issued.record.id, true).unwrap();
    drop(store);
    let c = rusqlite::Connection::open(&path).unwrap();
    c.execute_batch("ALTER TABLE api_tokens DROP COLUMN max_uses; ALTER TABLE api_tokens DROP COLUMN used_count;").unwrap();
    drop(c);
    let store = Store::open(&path).unwrap();
    let record = store.authorize(&issued.token, 101).unwrap();
    assert_eq!(record.max_uses, None);
    assert_eq!(record.used_count, 0);
    assert_eq!(record.success_count, 1);
}
