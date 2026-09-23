//! Narrow adapter to the existing active-account APIs; no account or task management.
use crate::error::Error;
use reqwest::{Client, Method};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    net::IpAddr,
    time::Duration,
};
use url::Url;

const MAX_FILES: usize = 20;
const MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DOWNLOAD_UA: &str = "netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInput {
    pub share_url: String,
    pub password: Option<String>,
    pub selected_fs_ids: Option<Vec<u64>>,
}
impl ResolveInput {
    pub fn validate(&self) -> Result<(), Error> {
        let url = Url::parse(&self.share_url).map_err(|_| Error::Input)?;
        let share_path = url.path().strip_prefix("/s/").is_some_and(|s| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        });
        let init_path = url.path() == "/share/init"
            && url.query_pairs().any(|(k, v)| {
                k == "surl"
                    && !v.is_empty()
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            });
        if self.share_url.len() > 2048
            || url.scheme() != "https"
            || url.host_str() != Some("pan.baidu.com")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.fragment().is_some()
            || !(share_path || init_path)
            || url.query_pairs().any(|(k, _)| k != "pwd" && k != "surl")
            || self
                .password
                .as_ref()
                .is_some_and(|p| p.len() != 4 || !p.bytes().all(|b| b.is_ascii_alphanumeric()))
            || self.selected_fs_ids.as_ref().is_some_and(|ids| {
                ids.is_empty()
                    || ids.len() > MAX_FILES
                    || ids.contains(&0)
                    || ids.iter().collect::<HashSet<_>>().len() != ids.len()
            })
        {
            return Err(Error::Input);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ShareFile {
    pub fs_id: u64,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Preview {
    pub files: Vec<ShareFile>,
}
#[derive(Debug, Serialize)]
pub struct LinkFile {
    pub filename: String,
    pub size: u64,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub expires_at: Option<i64>,
}
#[derive(Debug, Serialize)]
pub struct ResolveResult {
    pub task_id: String,
    pub save_path: String,
    pub files: Vec<LinkFile>,
}
#[derive(Clone)]
pub struct Upstream {
    base: Url,
    client: Client,
}
#[derive(Deserialize)]
struct Envelope<T> {
    code: i32,
    data: Option<T>,
}
#[derive(Deserialize)]
struct Created {
    task_id: Option<String>,
    need_password: bool,
}
#[derive(Deserialize)]
struct Task {
    status: String,
    transferred_count: usize,
    total_count: usize,
}
#[derive(Deserialize)]
struct FileList {
    list: Vec<SavedFile>,
    has_more: bool,
}
#[derive(Deserialize)]
struct SavedFile {
    fs_id: u64,
    path: String,
    server_filename: String,
    size: u64,
    isdir: i32,
}
#[derive(Deserialize)]
struct Download {
    url: String,
}
impl Upstream {
    pub fn new(base: &str, bearer: Option<&str>) -> Result<Self, Error> {
        let base = Url::parse(base).map_err(|_| Error::Input)?;
        if base.scheme() != "http"
            || !base
                .host_str()
                .and_then(|h| h.trim_matches(['[', ']']).parse::<IpAddr>().ok())
                .is_some_and(|ip| ip.is_loopback())
            || base.path() != "/"
            || base.query().is_some()
            || base.fragment().is_some()
            || !base.username().is_empty()
            || base.password().is_some()
            || base.port() == Some(0)
        {
            return Err(Error::Input);
        }
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(bearer) = bearer {
            if bearer.is_empty() {
                return Err(Error::Input);
            }
            let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {bearer}"))
                .map_err(|_| Error::Input)?;
            value.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|_| Error::Input)?;
        Ok(Self { base, client })
    }
    async fn call<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<T, Error> {
        let url = self.base.join(path).map_err(|_| Error::Upstream)?;
        let mut request = self.client.request(method, url).query(query);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let mut response = request.send().await.map_err(|_| Error::Upstream)?;
        if !response.status().is_success() {
            return Err(Error::Upstream);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Upstream)? {
            if bytes.len() + chunk.len() > 1024 * 1024 {
                return Err(Error::Upstream);
            }
            bytes.extend_from_slice(&chunk);
        }
        let result: Envelope<T> = serde_json::from_slice(&bytes).map_err(|_| Error::Upstream)?;
        match result.code {
            0 => result.data.ok_or(Error::Upstream),
            1001 => Err(Error::PasswordRequired),
            1002 => Err(Error::PasswordInvalid),
            1003 | 1004 => Err(Error::ShareUnavailable),
            1007..=1009 => Err(Error::TransferFailed),
            _ => Err(Error::Upstream),
        }
    }
    pub async fn preview(&self, input: &ResolveInput) -> Result<Preview, Error> {
        input.validate()?;
        // One extra entry detects "all" overflowing the first-version limit.
        let preview: Preview=self.call(Method::POST,"/api/v1/transfers/preview",&[],Some(json!({"share_url":input.share_url,"password":input.password,"page":1,"num":MAX_FILES+1}))).await?;
        if preview.files.len() > MAX_FILES + 1 {
            return Err(Error::FileLimit);
        }
        Ok(preview)
    }
    pub async fn resolve(&self, input: &ResolveInput) -> Result<ResolveResult, Error> {
        tokio::time::timeout(Duration::from_secs(180), self.resolve_inner(input))
            .await
            .map_err(|_| Error::Timeout)?
    }
    async fn resolve_inner(&self, input: &ResolveInput) -> Result<ResolveResult, Error> {
        let preview = self.preview(input).await?;
        let selected: Vec<_> = preview
            .files
            .into_iter()
            .filter(|f| {
                input
                    .selected_fs_ids
                    .as_ref()
                    .is_none_or(|ids| ids.contains(&f.fs_id))
            })
            .collect();
        let size = selected
            .iter()
            .try_fold(0u64, |sum, f| sum.checked_add(f.size))
            .ok_or(Error::FileLimit)?;
        if selected.is_empty()
            || selected.len() > MAX_FILES
            || size > MAX_BYTES
            || selected.iter().any(|f| f.is_dir)
            || input
                .selected_fs_ids
                .as_ref()
                .is_some_and(|ids| ids.len() != selected.len())
        {
            return Err(Error::FileLimit);
        }
        // Root UUID folder: avoids parent mkdir conflicts and any caller-controlled path.
        let save_path = format!("/.bpr_directlink_api_{}", uuid::Uuid::new_v4());
        let created:Created=self.call(Method::POST,"/api/v1/transfers",&[],Some(json!({
            "share_url":input.share_url,"password":input.password,"save_path":save_path,"save_fs_id":0,
            "auto_download":false,"is_share_direct_download":false,
            "selected_fs_ids":selected.iter().map(|f|f.fs_id).collect::<Vec<_>>(),"selected_files":selected
        }))).await?;
        if created.need_password {
            return Err(Error::PasswordRequired);
        }
        let task_id = created
            .task_id
            .filter(|id| {
                !id.is_empty()
                    && id.len() <= 128
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            })
            .ok_or(Error::Upstream)?;
        loop {
            let task: Task = self
                .call(
                    Method::GET,
                    &format!("/api/v1/transfers/{task_id}"),
                    &[],
                    None,
                )
                .await?;
            match task.status.as_str() {
                "transferred" | "completed" => {
                    if task.transferred_count != selected.len()
                        || task.total_count != selected.len()
                    {
                        return Err(Error::TransferFailed);
                    }
                    break;
                }
                "queued" | "checking_share" | "transferring" => {
                    tokio::time::sleep(Duration::from_secs(1)).await
                }
                _ => return Err(Error::TransferFailed),
            }
        }
        let listing: FileList = self
            .call(
                Method::GET,
                "/api/v1/files",
                &[
                    ("dir", save_path.clone()),
                    ("page_size", (MAX_FILES + 1).to_string()),
                ],
                None,
            )
            .await?;
        if listing.has_more || listing.list.len() != selected.len() {
            return Err(Error::TransferFailed);
        }
        let mut files = Vec::new();
        let mut seen = HashSet::new();
        for file in listing.list {
            if file.isdir != 0
                || file.fs_id == 0
                || !seen.insert(file.fs_id)
                || file.server_filename.contains(['/', '\\'])
                || matches!(file.server_filename.as_str(), "" | "." | "..")
                || file.path != format!("{}/{}", save_path, file.server_filename)
                || !selected
                    .iter()
                    .any(|f| f.name == file.server_filename && f.size == file.size)
            {
                return Err(Error::Upstream);
            }
            let download: Download = self
                .call(
                    Method::GET,
                    "/api/v1/files/download",
                    &[("fs_id", file.fs_id.to_string()), ("path", file.path)],
                    None,
                )
                .await?;
            let link = Url::parse(&download.url).map_err(|_| Error::Upstream)?;
            if !matches!(link.scheme(), "http" | "https")
                || link.host_str().is_none()
                || !link.username().is_empty()
                || link.password().is_some()
            {
                return Err(Error::Upstream);
            }
            files.push(LinkFile {
                filename: file.server_filename,
                size: file.size,
                url: download.url,
                headers: BTreeMap::from([("User-Agent".into(), DOWNLOAD_UA.into())]),
                expires_at: None,
            });
        }
        Ok(ResolveResult {
            task_id,
            save_path,
            files,
        })
    }
}
