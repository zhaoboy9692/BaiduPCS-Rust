use axum::{
    extract::{Request, State},
    Json, Router,
};
use baidupcs_directlink_api::upstream::{ResolveInput, Upstream};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fake {
    calls: Vec<(String, Value)>,
    saved: String,
    mode: String,
}
async fn handler(State(state): State<Arc<Mutex<Fake>>>, req: Request) -> Json<Value> {
    let path = req.uri().path().to_owned();
    let query = req.uri().query().unwrap_or("").to_owned();
    let body = req.into_body().collect().await.unwrap().to_bytes();
    let input: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let mut s = state.lock().unwrap();
    s.calls.push((path.clone(), input.clone()));
    if path.ends_with("/preview") {
        if s.mode == "password" {
            return Json(json!({"code":1002,"message":"secret BDUSS upstream detail"}));
        }
        return Json(
            json!({"code":0,"data":{"files":[{"fs_id":1,"name":"a.txt","path":"/a.txt","is_dir":false,"size":3}],"share_info":{"bdstoken":"PRIVATE","token":"PRIVATE"}}}),
        );
    }
    let data = match path.as_str() {
        "/api/v1/transfers" => {
            s.saved = input["save_path"].as_str().unwrap().into();
            json!({"task_id":"original-task-1","status":"queued","need_password":false})
        }
        "/api/v1/transfers/original-task-1" => {
            json!({"id":"original-task-1","status":"transferred","transferred_count":1,"total_count":1})
        }
        "/api/v1/files" => {
            let path = if s.mode == "escape" {
                "/outside/a.txt".into()
            } else {
                format!("{}/a.txt", s.saved)
            };
            json!({"list":[{"fs_id":2,"path":path,"server_filename":"a.txt","size":3,"isdir":0}],"has_more":false})
        }
        "/api/v1/files/download" => {
            assert!(query.contains("fs_id=2"));
            json!({"fs_id":2,"url":"https://d.pcs.baidu.com/file?sign=test"})
        }
        _ => panic!("unexpected upstream API {path}"),
    };
    Json(json!({"code":0,"data":data}))
}
async fn setup(mode: &str) -> (Upstream, Arc<Mutex<Fake>>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mutex::new(Fake {
        mode: mode.into(),
        ..Default::default()
    }));
    let app = Router::new().fallback(handler).with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (Upstream::new(&url, None).unwrap(), state, task)
}
fn input() -> ResolveInput {
    serde_json::from_value(
        json!({"share_url":"https://pan.baidu.com/s/1abc","selected_fs_ids":[1]}),
    )
    .unwrap()
}
#[tokio::test]
async fn resolves_with_existing_transfer_without_download_or_uid_override() {
    let (upstream, state, task) = setup("").await;
    let preview = serde_json::to_string(&upstream.preview(&input()).await.unwrap()).unwrap();
    assert!(!preview.contains("PRIVATE"));
    let result = upstream.resolve(&input()).await.unwrap();
    assert_eq!(result.task_id, "original-task-1");
    assert_eq!(result.files[0].filename, "a.txt");
    assert!(result.files[0].headers.contains_key("User-Agent"));
    assert!(!result.files[0].headers.contains_key("Cookie"));
    let s = state.lock().unwrap();
    let transfer = &s
        .calls
        .iter()
        .find(|(p, _)| p == "/api/v1/transfers")
        .unwrap()
        .1;
    assert_eq!(transfer["auto_download"], false);
    assert_eq!(transfer["is_share_direct_download"], false);
    assert!(transfer.get("uid").is_none());
    assert!(s.saved.starts_with("/.bpr_directlink_api_"));
    task.abort();
}
#[tokio::test]
async fn errors_are_sanitized_and_paths_cannot_escape_saved_directory() {
    for mode in ["password", "escape"] {
        let (upstream, state, task) = setup(mode).await;
        let err = upstream.resolve(&input()).await.unwrap_err();
        assert!(!err.to_string().contains("BDUSS"));
        assert!(!state
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(p, _)| p.ends_with("/download")));
        if mode == "password" {
            assert_eq!(err.code(), "share_password_invalid");
        }
        task.abort();
    }
}
#[test]
fn only_fixed_loopback_upstream_is_allowed() {
    for url in [
        "http://example.com",
        "http://127.0.0.1/evil",
        "http://u:p@127.0.0.1",
        "http://127.0.0.1?url=x",
    ] {
        assert!(Upstream::new(url, None).is_err());
    }
}
