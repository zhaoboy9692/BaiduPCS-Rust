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
    chosen: Value,
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
        if s.mode == "mixed" {
            return Json(
                json!({"code":0,"data":{"files":[{"fs_id":10,"name":"bad.txt","path":"/bad.txt","is_dir":false,"size":3},{"fs_id":1,"name":"a.txt","path":"/a.txt","is_dir":false,"size":3}]}}),
            );
        }
        if ["nested", "deep", "dirfail"].contains(&s.mode.as_str()) {
            return Json(
                json!({"code":0,"data":{"files":[{"fs_id":99,"name":"folder","path":"/original/folder","is_dir":true,"size":0}],"share_info":{"short_key":"1abc","shareid":"123","uk":"456","bdstoken":"PRIVATE","kind":"personal","token":"PRIVATE"}}}),
            );
        }
        if s.mode == "many" {
            let files: Vec<_> = (1..=101).map(|i| json!({"fs_id":i,"name":format!("{i}.bin"),"path":format!("/{i}.bin"),"is_dir":false,"size":5_000_000_000u64})).collect();
            let page = input["page"].as_u64().unwrap_or(1) as usize;
            let num = input["num"].as_u64().unwrap() as usize;
            return Json(
                json!({"code":0,"data":{"files":files.into_iter().skip((page-1)*num).take(num).collect::<Vec<_>>()}}),
            );
        }
        return Json(
            json!({"code":0,"data":{"files":[{"fs_id":1,"name":"a.txt","path":"/a.txt","is_dir":false,"size":3}],"share_info":{"bdstoken":"PRIVATE","token":"PRIVATE"}}}),
        );
    }
    let data = match path.as_str() {
        "/api/v1/transfers/preview/dir" => {
            if s.mode == "dirfail" {
                return Json(json!({"code":-1,"message":"secret"}));
            }
            if s.mode == "deep" {
                return if input["dir"] == "/sharelink456-123/folder" {
                    Json(
                        json!({"code":0,"data":{"files":[{"fs_id":98,"name":"inner","path":"/sharelink456-123/folder/inner","is_dir":true,"size":0}]}}),
                    )
                } else {
                    assert_eq!(input["dir"], "/sharelink456-123/folder/inner");
                    Json(
                        json!({"code":0,"data":{"files":[{"fs_id":1,"name":"a.txt","path":"/sharelink456-123/folder/inner/a.txt","is_dir":false,"size":3}]}}),
                    )
                };
            }
            assert_eq!(input["dir"], "/sharelink456-123/folder");
            assert_eq!(input["bdstoken"], "PRIVATE");
            json!({"files":[{"fs_id":1,"name":"a.txt","path":"/sharelink456-123/folder/a.txt","is_dir":false,"size":3}]})
        }
        "/api/v1/files/folder" => {
            s.saved = input["path"].as_str().unwrap().into();
            json!({"fs_id":90,"path":s.saved,"isdir":1})
        }
        "/api/v1/transfers" => {
            s.saved = input["save_path"].as_str().unwrap().into();
            assert_eq!(input["selected_fs_ids"].as_array().unwrap().len(), 1);
            s.chosen = input["selected_files"][0].clone();
            if s.mode == "mixed" && s.chosen["fs_id"] == 10 {
                return Json(json!({"code":1007,"message":"private upstream detail"}));
            }
            json!({"task_id":"original-task-1","status":"queued","need_password":false})
        }
        "/api/v1/transfers/original-task-1" => {
            json!({"id":"original-task-1","status":"transferred","transferred_count":1,"total_count":1})
        }
        "/api/v1/files" => {
            let path = if s.mode == "escape" {
                "/outside/a.txt".into()
            } else {
                format!("{}/{}", s.saved, s.chosen["name"].as_str().unwrap())
            };
            json!({"list":[{"fs_id":2,"path":path,"server_filename":s.chosen["name"],"size":s.chosen["size"],"isdir":0}],"has_more":false})
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
    assert_eq!(
        result.list[0].task_id.as_deref().unwrap(),
        "original-task-1"
    );
    assert_eq!(result.list[0].name, "a.txt");
    assert!(result.list[0].headers.contains_key("User-Agent"));
    assert!(!result.list[0].headers.contains_key("Cookie"));
    let s = state.lock().unwrap();
    let transfer = &s
        .calls
        .iter()
        .find(|(p, _)| p == "/api/v1/transfers")
        .unwrap()
        .1;
    let mkdir = s
        .calls
        .iter()
        .position(|(p, _)| p == "/api/v1/files/folder");
    let transfer_pos = s
        .calls
        .iter()
        .position(|(p, _)| p == "/api/v1/transfers")
        .unwrap();
    assert!(
        mkdir.is_some_and(|i| i < transfer_pos),
        "destination must be created before transfer, including v2.2.4 release binary"
    );
    assert_eq!(transfer["save_fs_id"], 90);
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
        let result = upstream.resolve(&input()).await;
        let err = if mode == "password" {
            result.unwrap_err().to_string()
        } else {
            result.unwrap().list[0]
                .error
                .as_ref()
                .unwrap()
                .message
                .clone()
        };
        assert!(!err.to_string().contains("BDUSS"));
        assert!(!state
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|(p, _)| p.ends_with("/download")));
        if mode == "password" {
            assert_eq!(err, "分享提取码错误");
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

#[tokio::test]
async fn nested_selected_file_uses_original_directory_api() {
    let (upstream, state, task) = setup("nested").await;
    let request: ResolveInput=serde_json::from_value(json!({"share_url":"https://pan.baidu.com/s/1abc","selected_fs_ids":[1],"selected_paths":["/sharelink456-123/folder/a.txt"]})).unwrap();
    let result = upstream.resolve(&request).await.unwrap();
    assert_eq!(result.list[0].name, "a.txt");
    assert!(!serde_json::to_string(&result).unwrap().contains("PRIVATE"));
    assert!(state
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|(p, _)| p.ends_with("/preview/dir")));
    task.abort();
}
#[tokio::test]
async fn no_count_or_size_limit_and_each_file_is_separate_including_later_page() {
    let (upstream, state, task) = setup("many").await;
    let request: ResolveInput=serde_json::from_value(json!({"share_url":"https://pan.baidu.com/s/1abc","selected_fs_ids":(80..=101).collect::<Vec<_>>()})).unwrap();
    let result = upstream.resolve(&request).await.unwrap();
    assert_eq!(result.list.len(), 22);
    assert_eq!(result.list[0].size, 5_000_000_000);
    assert_eq!(
        state
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|(p, _)| p == "/api/v1/transfers")
            .count(),
        22
    );
    task.abort();
}
#[tokio::test]
async fn unknown_selection_is_reported_without_transfer() {
    let (upstream, state, task) = setup("").await;
    let request = serde_json::from_value(
        json!({"share_url":"https://pan.baidu.com/s/1abc","selected_fs_ids":[42]}),
    )
    .unwrap();
    let result = upstream.resolve(&request).await.unwrap();
    assert_eq!(
        result.list[0].error.as_ref().unwrap().code,
        "selection_not_found"
    );
    assert!(!state
        .lock()
        .unwrap()
        .calls
        .iter()
        .any(|(p, _)| p == "/api/v1/transfers"));
    task.abort();
}
#[tokio::test]
async fn no_selection_recurses_and_selected_folder_also_recurses() {
    for ids in [Value::Null, json!([99])] {
        let (upstream, _, task) = setup("nested").await;
        let request = serde_json::from_value(
            json!({"share_url":"https://pan.baidu.com/s/1abc","selected_fs_ids":ids}),
        )
        .unwrap();
        let result = upstream.resolve(&request).await.unwrap();
        assert_eq!(result.list.len(), 1);
        assert_eq!(result.list[0].name, "a.txt");
        assert!(result.list[0].success);
        task.abort();
    }
}
#[tokio::test]
async fn one_failure_does_not_stop_the_rest_and_returns_json_list() {
    let (upstream, _, task) = setup("mixed").await;
    let request =
        serde_json::from_value(json!({"share_url":"https://pan.baidu.com/s/1abc"})).unwrap();
    let result = upstream.resolve(&request).await.unwrap();
    assert_eq!(result.list.len(), 2);
    assert!(!result.list[0].success);
    assert!(result.list[0].url.is_none());
    assert!(result.list[0].error.is_some());
    assert!(result.list[1].success);
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["list"][1]["size"], 3);
    assert_eq!(json["failed"], 1);
    assert_eq!(json["succeeded"], 1);
    task.abort();
}

#[tokio::test]
async fn http_api_returns_top_level_json_list_with_mixed_results() {
    use axum::{body::Body, http::Request};
    use baidupcs_directlink_api::{
        http::{router, Config},
        store::Store,
    };
    use tower::ServiceExt;
    let (upstream, _, task) = setup("mixed").await;
    let app = router(
        Store::open_memory().unwrap(),
        Config::new("a".repeat(32), vec!["http://example.test".into()])
            .unwrap()
            .with_upstream(upstream),
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/direct-admin/v1/resolve")
                .header("content-type", "application/json")
                .header("x-directlink-admin-key", "a".repeat(32))
                .body(Body::from(
                    r#"{"share_url":"https://pan.baidu.com/s/1abc"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "application/json");
    let json: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["list"].as_array().unwrap().len(), 2);
    assert_eq!(json["failed"], 1);
    assert_eq!(json["succeeded"], 1);
    assert_eq!(json["complete"], true);
    task.abort();
}

#[tokio::test]
async fn public_batch_reserves_quota_per_file_and_does_not_transfer_rejected_files() {
    use axum::{body::Body, http::Request};
    use baidupcs_directlink_api::{
        http::{router, Config},
        store::Store,
    };
    use tower::ServiceExt;
    let (upstream, state, task) = setup("mixed").await;
    let store = Store::open_memory().unwrap();
    let issued = store
        .create(
            serde_json::from_value(json!({"name":"quota","max_uses":1})).unwrap(),
            1,
        )
        .unwrap();
    let app = router(
        store,
        Config::new("a".repeat(32), vec!["http://example.test".into()])
            .unwrap()
            .with_upstream(upstream),
    );
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/direct-api/v1/resolve")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", issued.token))
                .body(Body::from(
                    r#"{"share_url":"https://pan.baidu.com/s/1abc"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let json: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(json["list"][0]["error"]["code"], "transfer_failed");
    assert_eq!(json["list"][1]["error"]["code"], "token_quota_exhausted");
    assert_eq!(
        state
            .lock()
            .unwrap()
            .calls
            .iter()
            .filter(|(p, _)| p == "/api/v1/transfers")
            .count(),
        1
    );
    task.abort();
}

#[tokio::test]
async fn deeply_nested_files_and_directory_failure_are_not_silently_omitted() {
    for mode in ["deep", "dirfail"] {
        let (upstream, _, task) = setup(mode).await;
        let request =
            serde_json::from_value(json!({"share_url":"https://pan.baidu.com/s/1abc"})).unwrap();
        let result = upstream.resolve(&request).await.unwrap();
        assert_eq!(result.list.len(), 1);
        if mode == "deep" {
            assert!(result.list[0].success);
            assert!(result.complete);
        } else {
            assert!(!result.complete);
            assert_eq!(
                result.list[0].error.as_ref().unwrap().code,
                "directory_failed"
            );
        }
        task.abort();
    }
}
