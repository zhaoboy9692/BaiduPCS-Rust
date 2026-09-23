//! Narrow adapter to the existing active-account APIs; no account or task management.
use crate::{error::Error, store::Store};
use reqwest::{Client, Method};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    net::IpAddr,
    sync::Arc,
    time::Duration,
};
use url::Url;

const PAGE_SIZE: usize = 100;
const DOWNLOAD_UA: &str = "netdisk;P2SP;3.0.0.8;netdisk;11.12.3;ANG-AN00;android-android;10.0;JSbridge4.4.0;jointBridge;1.1.0;";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveInput {
    pub share_url: String,
    pub password: Option<String>,
    pub selected_fs_ids: Option<Vec<u64>>,
    /// Untrusted traversal hints only; never used as file metadata or credentials.
    pub selected_paths: Option<Vec<String>>,
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
            || self.selected_paths.as_ref().is_some_and(|paths| {
                paths.iter().any(|p| {
                    !p.starts_with('/')
                        || p.len() > 4096
                        || p.contains('\0')
                        || p.split('/').any(|part| part == "..")
                })
            })
            || self.selected_fs_ids.as_ref().is_some_and(|ids| {
                ids.is_empty()
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
#[derive(Debug, Clone, Serialize)]
pub struct ItemError {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct LinkFile {
    pub fs_id: u64,
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub success: bool,
    pub url: Option<String>,
    pub headers: BTreeMap<String, String>,
    pub expires_at: Option<i64>,
    pub task_id: Option<String>,
    pub save_path: Option<String>,
    pub error: Option<ItemError>,
}
impl LinkFile {
    fn pending(file: &ShareFile) -> Self {
        Self {
            fs_id: file.fs_id,
            name: file.name.clone(),
            path: file.path.clone(),
            is_dir: file.is_dir,
            size: file.size,
            success: false,
            url: None,
            headers: BTreeMap::new(),
            expires_at: None,
            task_id: None,
            save_path: None,
            error: None,
        }
    }
    fn fail(&mut self, error: Error) {
        self.error = Some(ItemError {
            code: error.code().into(),
            message: error.to_string(),
        });
    }
}
#[derive(Debug, Serialize)]
pub struct ResolveResult {
    pub list: Vec<LinkFile>,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    /// False if a directory could not be fully enumerated.
    pub complete: bool,
}
#[derive(Deserialize)]
struct RawPreview {
    files: Vec<ShareFile>,
    share_info: Option<Value>,
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
struct Folder {
    fs_id: u64,
    path: String,
    isdir: i32,
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
    error: Option<String>,
}
impl Task {
    fn failure(&self) -> Error {
        // Return only allowlisted messages. Raw task errors may contain cookies
        // or signed URLs and must never be forwarded to public API callers.
        let reason = self.error.as_deref().unwrap_or_default();
        if reason.contains("登录已过期") || reason.contains("凭证不完整") {
            Error::UpstreamLoginRequired
        } else if reason.contains("转存路径不存在") || reason.contains("目标目录不存在")
        {
            Error::TransferPathMissing
        } else {
            Error::TransferFailed
        }
    }
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
    async fn page(
        &self,
        input: &ResolveInput,
        info: Option<&Value>,
        dir: Option<&str>,
        page: u32,
    ) -> Result<RawPreview, Error> {
        let (path, body) = if let Some(dir) = dir {
            let mut body = info.cloned().ok_or(Error::Upstream)?;
            let object = body.as_object_mut().ok_or(Error::Upstream)?;
            object.insert("dir".into(), json!(dir));
            object.insert("page".into(), json!(page));
            object.insert("num".into(), json!(PAGE_SIZE));
            ("/api/v1/transfers/preview/dir", body)
        } else {
            (
                "/api/v1/transfers/preview",
                json!({"share_url":input.share_url,"password":input.password,"page":page,"num":PAGE_SIZE}),
            )
        };
        self.call(Method::POST, path, &[], Some(body)).await
    }
    pub async fn preview(&self, input: &ResolveInput) -> Result<Preview, Error> {
        input.validate()?;
        let (files, failures) = self.discover(input).await?;
        if !failures.is_empty() {
            return Err(Error::DirectoryFailed);
        }
        Ok(Preview { files })
    }
    async fn discover(
        &self,
        input: &ResolveInput,
    ) -> Result<(Vec<ShareFile>, Vec<LinkFile>), Error> {
        input.validate()?;
        let first = self.page(input, None, None, 1).await?;
        let info = first.share_info.clone();
        // Queue entries are always discovered from trusted upstream listings, never user paths.
        let mut queue = VecDeque::from([(
            None::<String>,
            None::<ShareFile>,
            input.selected_fs_ids.is_none(),
        )]);
        let mut first = Some(first);
        let mut visited_dirs = HashSet::new();
        let mut seen_files = HashSet::new();
        let mut matched = HashSet::new();
        let mut files = Vec::new();
        let mut failures = Vec::new();
        while let Some((dir, folder, include_all)) = queue.pop_front() {
            let mut page = 1u32;
            let mut seen_entries = HashSet::new();
            loop {
                let response = if dir.is_none() && page == 1 {
                    Ok(first.take().ok_or(Error::Upstream)?)
                } else {
                    self.page(input, info.as_ref(), dir.as_deref(), page).await
                };
                let listing = match response {
                    Ok(v) => v,
                    Err(e) => {
                        if let Some(folder) = &folder {
                            let mut item = LinkFile::pending(folder);
                            item.fail(Error::DirectoryFailed);
                            failures.push(item);
                            break;
                        }
                        return Err(e);
                    }
                };
                let count = listing.files.len();
                let mut new_entries = 0;
                for file in listing.files {
                    if file.fs_id == 0
                        || file.name.is_empty()
                        || file.name.contains(['/', '\\'])
                        || matches!(file.name.as_str(), "." | "..")
                    {
                        return Err(Error::Upstream);
                    }
                    if !seen_entries.insert(file.fs_id) {
                        continue;
                    }
                    new_entries += 1;
                    let explicitly_selected = input
                        .selected_fs_ids
                        .as_ref()
                        .is_some_and(|ids| ids.contains(&file.fs_id));
                    if explicitly_selected {
                        matched.insert(file.fs_id);
                    }
                    let selected = include_all || explicitly_selected;
                    if file.is_dir {
                        let info = info.as_ref().ok_or(Error::Upstream)?;
                        let child =
                            if info["kind"] == "apaas" || file.path.starts_with("/sharelink") {
                                file.path.clone()
                            } else if let Some(parent) = &dir {
                                format!("{parent}/{}", file.name)
                            } else {
                                format!(
                                    "/sharelink{}-{}/{}",
                                    info["uk"].as_str().ok_or(Error::Upstream)?,
                                    info["shareid"].as_str().ok_or(Error::Upstream)?,
                                    file.name
                                )
                            };
                        // Hints only narrow a traversal; IDs and sizes still come from original API.
                        let relevant = selected
                            || input.selected_paths.as_ref().is_none_or(|paths| {
                                paths.iter().any(|p| {
                                    p == &child
                                        || p.starts_with(&format!("{child}/"))
                                        || p == &file.path
                                        || p.starts_with(&format!("{}/", file.path))
                                })
                            });
                        if relevant && visited_dirs.insert(file.fs_id) {
                            queue.push_back((Some(child), Some(file), selected));
                        }
                    } else if selected && seen_files.insert(file.fs_id) {
                        files.push(file);
                    }
                }
                if count < PAGE_SIZE {
                    break;
                }
                if new_entries == 0 {
                    return Err(Error::DirectoryFailed);
                }
                page = page.checked_add(1).ok_or(Error::DirectoryFailed)?;
            }
        }
        if let Some(ids) = &input.selected_fs_ids {
            for id in ids {
                if !matched.contains(id) {
                    let mut item = LinkFile::pending(&ShareFile {
                        fs_id: *id,
                        name: format!("文件 {id}"),
                        path: String::new(),
                        is_dir: false,
                        size: 0,
                    });
                    item.fail(Error::SelectionNotFound);
                    failures.push(item);
                }
            }
        }
        Ok((files, failures))
    }
    pub async fn resolve(&self, input: &ResolveInput) -> Result<ResolveResult, Error> {
        self.resolve_with_progress(input, None, None).await
    }
    pub async fn resolve_with_progress(
        &self,
        input: &ResolveInput,
        progress: Option<tokio::sync::mpsc::Sender<LinkFile>>,
        quota: Option<(Arc<Store>, String, String)>,
    ) -> Result<ResolveResult, Error> {
        let (files, failures) = self.discover(input).await?;
        let complete = !failures.iter().any(|f| f.is_dir);
        let mut list = Vec::new();
        for item in failures {
            if let Some(tx) = &progress {
                if tx.send(item.clone()).await.is_err() {
                    return Err(Error::Timeout);
                }
            }
            list.push(item);
        }
        for file in files {
            if progress.as_ref().is_some_and(|tx| tx.is_closed()) {
                return Err(Error::Timeout);
            }
            let mut item = LinkFile::pending(&file);
            if let Some((store, id, secret)) = &quota {
                let store = store.clone();
                let id = id.clone();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let secret = secret.clone();
                let reservation = tokio::task::spawn_blocking(move || {
                    store.reserve_authenticated(&id, &secret, now)
                })
                .await
                .map_err(|_| Error::Storage)?;
                if let Err(e) = reservation {
                    item.fail(e);
                    if let Some(tx) = &progress {
                        if tx.send(item.clone()).await.is_err() {
                            return Err(Error::Timeout);
                        }
                    }
                    list.push(item);
                    continue;
                }
            }

            let outcome = tokio::time::timeout(
                Duration::from_secs(180),
                self.resolve_one(input, &file, &mut item),
            )
            .await
            .unwrap_or(Err(Error::Timeout));
            if let Err(e) = outcome {
                item.fail(e);
            }
            if let Some((store, id, _)) = &quota {
                let store = store.clone();
                let id = id.clone();
                let success = item.success;
                tokio::task::spawn_blocking(move || store.record_result(&id, success))
                    .await
                    .map_err(|_| Error::Storage)??;
            }

            if let Some(tx) = &progress {
                if tx.send(item.clone()).await.is_err() {
                    return Err(Error::Timeout);
                }
            }
            list.push(item);
        }
        let succeeded = list.iter().filter(|f| f.success).count();
        Ok(ResolveResult {
            total: list.len(),
            failed: list.len() - succeeded,
            succeeded,
            list,
            complete,
        })
    }
    async fn resolve_one(
        &self,
        input: &ResolveInput,
        selected_file: &ShareFile,
        item: &mut LinkFile,
    ) -> Result<(), Error> {
        // v2.2.4 uses selected_files.path only to group destination directories;
        // the actual share transfer identifies the source by fs_id. Each request
        // here contains one ordinary file in a fresh UUID directory, so flatten
        // that destination metadata instead of reproducing the share hierarchy.
        // Keep the authoritative source path on `item` for callers. This also
        // avoids the backend's missing-child-directory mkdir/listing ambiguity.
        let mut transfer_file = selected_file.clone();
        transfer_file.path = format!("/{}", selected_file.name);
        let selected = std::slice::from_ref(&transfer_file);
        // Root UUID folder: avoids parent mkdir conflicts and any caller-controlled path.
        let save_path = format!("/.bpr_directlink_api_{}", uuid::Uuid::new_v4());
        item.save_path = Some(save_path.clone());
        let folder: Folder = self
            .call(
                Method::POST,
                "/api/v1/files/folder",
                &[],
                Some(json!({"path":save_path})),
            )
            .await?;
        if folder.path != save_path || folder.fs_id == 0 || folder.isdir != 1 {
            return Err(Error::Upstream);
        }
        let created:Created=self.call(Method::POST,"/api/v1/transfers",&[],Some(json!({
            "share_url":input.share_url,"password":input.password,"save_path":save_path,"save_fs_id":folder.fs_id,
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
        item.task_id = Some(task_id.clone());
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
                        return Err(task.failure());
                    }
                    break;
                }
                "queued" | "checking_share" | "transferring" => {
                    tokio::time::sleep(Duration::from_secs(1)).await
                }
                _ => return Err(task.failure()),
            }
        }
        let listing: FileList = self
            .call(
                Method::GET,
                "/api/v1/files",
                &[("dir", save_path.clone()), ("page_size", "2".into())],
                None,
            )
            .await?;
        if listing.has_more || listing.list.len() != selected.len() {
            return Err(Error::TransferFailed);
        }
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
            item.success = true;
            item.url = Some(download.url);
            item.headers = BTreeMap::from([("User-Agent".into(), DOWNLOAD_UA.into())]);
        }
        Ok(())
    }
}
