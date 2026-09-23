use baidupcs_directlink_api::runtime::RuntimeConfig;
#[test]
fn rejects_public_bind_and_invalid_origins() {
    let base = serde_json::json!({"listen":"127.0.0.1:18889","database":"/tmp/token.sqlite","admin_key":"01234567890123456789012345678901","origins":["http://example.test","https://example.test"]});
    let config: RuntimeConfig = serde_json::from_value(base.clone()).unwrap();
    assert!(config.validate().is_ok());
    for address in ["0.0.0.0:18889", "192.168.1.1:18889", "[::]:18889"] {
        let mut bad = base.clone();
        bad["listen"] = address.into();
        let config: RuntimeConfig = serde_json::from_value(bad).unwrap();
        assert!(config.validate().is_err());
    }
    for origin in [
        "*",
        "http://example.test/path",
        "https://user:pass@example.test",
        "http://example.test?x=1",
    ] {
        let mut bad = base.clone();
        bad["origins"] = serde_json::json!([origin]);
        let config: RuntimeConfig = serde_json::from_value(bad).unwrap();
        assert!(config.validate().is_err());
    }
}
#[cfg(unix)]
#[test]
fn private_store_permissions_and_symlink_refusal() {
    use baidupcs_directlink_api::runtime::prepare_database;
    use std::os::unix::fs::{symlink, PermissionsExt};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let path = root.join("data/tokens.sqlite");
    prepare_database(&path).unwrap();
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    let link = root.join("link.sqlite");
    symlink(&path, &link).unwrap();
    assert!(prepare_database(&link).is_err());
}
