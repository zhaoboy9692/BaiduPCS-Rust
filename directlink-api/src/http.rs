use crate::{
    error::Error,
    store::{CreateToken, EditToken, Store},
    upstream::{ResolveInput, Upstream},
};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;

pub struct Config {
    admin_key: String,
    origins: Vec<String>,
    upstream: Option<Upstream>,
}
impl Config {
    pub fn new(admin_key: String, origins: Vec<String>) -> Result<Self, Error> {
        if admin_key.len() < 32
            || origins.is_empty()
            || origins.iter().any(|s| {
                !(s.starts_with("https://") || s.starts_with("http://"))
                    || s.contains('*')
                    || s.ends_with('/')
            })
        {
            return Err(Error::Input);
        }
        Ok(Self {
            admin_key,
            origins,
            upstream: None,
        })
    }
    pub fn with_upstream(mut self, upstream: Upstream) -> Self {
        self.upstream = Some(upstream);
        self
    }
}
#[derive(Clone)]
struct App {
    store: Arc<Store>,
    config: Arc<Config>,
    resolver: Arc<tokio::sync::Semaphore>,
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
async fn db<T: Send + 'static>(
    app: App,
    f: impl FnOnce(&Store) -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    tokio::task::spawn_blocking(move || f(&app.store))
        .await
        .map_err(|_| Error::Storage)?
}
async fn admin(State(app): State<App>, request: axum::extract::Request, next: Next) -> Response {
    let headers = request.headers();
    let key = headers
        .get("x-directlink-admin-key")
        .map(|v| v.as_bytes())
        .unwrap_or_default();
    if !bool::from(key.ct_eq(app.config.admin_key.as_bytes())) {
        return Error::Admin.into_response();
    }
    if !origin_allowed(headers, &app.config) {
        return Error::Origin.into_response();
    }
    next.run(request).await
}
fn origin_allowed(headers: &HeaderMap, config: &Config) -> bool {
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site")
    {
        return false;
    }
    match headers.get("origin") {
        None => true,
        Some(v) => v
            .to_str()
            .is_ok_and(|s| config.origins.iter().any(|v| v == s)),
    }
}
async fn no_cache(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}
#[derive(Deserialize)]
struct Pagination {
    #[serde(default)]
    offset: i64,
    #[serde(default = "page_size")]
    limit: i64,
}
fn page_size() -> i64 {
    50
}
async fn list(
    State(app): State<App>,
    Query(p): Query<Pagination>,
) -> Result<Json<serde_json::Value>, Error> {
    let data = db(app, move |s| s.list(p.offset, p.limit)).await?;
    Ok(Json(json!({"data":data})))
}
fn parse<T: serde::de::DeserializeOwned>(
    body: Result<Json<T>, axum::extract::rejection::JsonRejection>,
) -> Result<T, Box<Response>> {
    body.map(|Json(v)| v).map_err(|e| {
        Box::new({
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                (
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Json(json!({"code":"body_too_large","message":"请求体过大"})),
                )
                    .into_response()
            } else {
                Error::Input.into_response()
            }
        })
    })
}
async fn create(
    State(app): State<App>,
    body: Result<Json<CreateToken>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let input = match parse(body) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match db(app, move |s| s.create(input, now())).await {
        Ok(data) => (StatusCode::CREATED, Json(json!({"data":data}))).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn edit(
    State(app): State<App>,
    Path(id): Path<String>,
    body: Result<Json<EditToken>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let input = match parse(body) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match db(app, move |s| s.edit(&id, input, now())).await {
        Ok(data) => Json(json!({"data":data})).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn revoke(
    State(app): State<App>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Error> {
    let data = db(app, move |s| s.revoke(&id, now())).await?;
    Ok(Json(json!({"data":data})))
}
#[derive(Clone)]
struct Caller(String);
async fn bearer(
    State(app): State<App>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let secret = match request
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
    {
        Some((_, secret)) if !secret.is_empty() => secret.to_owned(),
        _ => return Error::InvalidToken.into_response(),
    };
    let read = request.uri().path().ends_with("/preview");
    match db(app, move |s| s.admit(&secret, now(), read)).await {
        Ok(record) => {
            request.extensions_mut().insert(Caller(record.id));
        }
        Err(e) => return e.into_response(),
    }
    next.run(request).await
}
async fn preview_share(
    State(app): State<App>,
    body: Result<Json<ResolveInput>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let input = match parse(body) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    if let Err(e) = input.validate() {
        return e.into_response();
    }
    let Some(upstream) = &app.config.upstream else {
        return Error::Upstream.into_response();
    };
    let Ok(_permit) = app.resolver.try_acquire() else {
        return Error::Busy.into_response();
    };
    match upstream.preview(&input).await {
        Ok(data) => Json(json!({"data":data})).into_response(),
        Err(e) => e.into_response(),
    }
}
async fn resolve_share(
    State(app): State<App>,
    caller: Option<axum::Extension<Caller>>,
    body: Result<Json<ResolveInput>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let input = match parse(body) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    if let Err(e) = input.validate() {
        return e.into_response();
    }
    let Some(upstream) = &app.config.upstream else {
        return Error::Upstream.into_response();
    };
    let Ok(permit) = app.resolver.clone().try_acquire_owned() else {
        return Error::Busy.into_response();
    };
    let upstream = upstream.clone();
    // Keep the single permit until bounded work finishes even if the HTTP caller disconnects.
    // This is not a task queue; the original service still owns the transfer task.
    let work = tokio::spawn(async move {
        let _permit = permit;
        let result = upstream.resolve(&input).await;
        if let Some(axum::Extension(Caller(id))) = caller {
            let success = result.is_ok();
            db(app, move |s| s.record_result(&id, success)).await?;
        }
        result
    });
    match work.await {
        Ok(Ok(data)) => Json(json!({"data":data})).into_response(),
        Ok(Err(e)) => e.into_response(),
        Err(_) => Error::Upstream.into_response(),
    }
}
pub fn router(store: Store, config: Config) -> Router {
    let app = App {
        store: Arc::new(store),
        config: Arc::new(config),
        resolver: Arc::new(tokio::sync::Semaphore::new(1)),
    };
    let public = Router::new()
        .route("/direct-api/v1/shares/preview", post(preview_share))
        .route("/direct-api/v1/resolve", post(resolve_share))
        .route_layer(middleware::from_fn_with_state(app.clone(), bearer));
    Router::new()
        .route("/direct-admin/v1/tokens", get(list).post(create))
        .route("/direct-admin/v1/tokens/:id", patch(edit))
        .route("/direct-admin/v1/tokens/:id/revoke", post(revoke))
        .route("/direct-admin/v1/shares/preview", post(preview_share))
        .route("/direct-admin/v1/resolve", post(resolve_share))
        .route_layer(middleware::from_fn_with_state(app.clone(), admin))
        .merge(public)
        .fallback(|| async { Error::NotFound })
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn(no_cache))
        .with_state(app)
}
