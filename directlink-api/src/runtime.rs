use crate::{error::Error, http::Config};
use serde::Deserialize;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub listen: SocketAddr,
    pub database: PathBuf,
    pub admin_key: String,
    pub origins: Vec<String>,
}
impl RuntimeConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.listen.ip().is_loopback()
            || self.listen.port() == 0
            || !self.database.is_absolute()
        {
            return Err(Error::Input);
        }
        for origin in &self.origins {
            let url = url::Url::parse(origin).map_err(|_| Error::Input)?;
            if !matches!(url.scheme(), "http" | "https")
                || url.host_str().is_none()
                || url.path() != "/"
                || url.query().is_some()
                || url.fragment().is_some()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.origin().ascii_serialization() != *origin
            {
                return Err(Error::Input);
            }
        }
        Config::new(self.admin_key.clone(), self.origins.clone())?;
        Ok(())
    }
}
#[cfg(unix)]
pub fn prepare_database(path: &Path) -> std::io::Result<()> {
    use std::{
        fs::{self, OpenOptions},
        os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    };
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("database needs a dedicated directory"))?;
    // Refuse symlink path components; the directory must be dedicated to this service.
    for ancestor in path.ancestors() {
        if let Ok(meta) = fs::symlink_metadata(ancestor) {
            if meta.file_type().is_symlink() {
                return Err(std::io::Error::other("symlink database path refused"));
            }
        }
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700).create(parent)?;
    if fs::metadata(parent)?.permissions().mode() & 0o077 != 0 {
        // Do not chmod an existing shared directory such as /tmp or /var/lib.
        return Err(std::io::Error::other(
            "database directory must have mode 0700",
        ));
    }
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}
#[cfg(not(unix))]
pub fn prepare_database(_path: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "this service currently requires a Unix deployment",
    ))
}
