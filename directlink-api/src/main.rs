use baidupcs_directlink_api::{
    http::{router, Config},
    runtime::{prepare_database, RuntimeConfig},
    store::Store,
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: baidupcs-directlink-api /absolute/path/config.json")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0 {
            return Err("config must be a private regular file (0600)".into());
        }
    }
    let config: RuntimeConfig = serde_json::from_slice(&std::fs::read(path)?)?;
    config.validate()?;
    prepare_database(&config.database)?;
    let store = Store::open(&config.database)?;
    let mut http_config = Config::new(config.admin_key, config.origins)?;
    if let Some(url) = config.upstream_url {
        http_config = http_config.with_upstream(baidupcs_directlink_api::upstream::Upstream::new(
            &url,
            config.upstream_bearer.as_deref(),
        )?);
    }
    let app = router(store, http_config);
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    eprintln!("Directlink service listening on {}", config.listen);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
