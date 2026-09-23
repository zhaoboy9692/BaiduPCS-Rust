use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use baidupcs_directlink_api::{
    http::{router, Config},
    store::Store,
};
use http_body_util::BodyExt;
use tower::ServiceExt;
const KEY: &str = "test-admin-channel-secret-32-bytes-long";
fn app() -> axum::Router {
    router(
        Store::open_memory().unwrap(),
        Config::new(KEY.into(), vec!["http://example.test".into()]).unwrap(),
    )
}
fn request(method: &str, path: &str, key: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("x-directlink-admin-key", key)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}
#[tokio::test]
async fn admin_boundary_and_cross_origin_are_enforced() {
    let app = app();
    for req in [
        request("GET", "/direct-admin/v1/tokens", "", ""),
        Request::builder()
            .uri("/direct-admin/v1/tokens")
            .header("Authorization", "Bearer dl_fake")
            .body(Body::empty())
            .unwrap(),
    ] {
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let mut req = request("POST", "/direct-admin/v1/tokens", KEY, r#"{"name":"test"}"#);
    req.headers_mut()
        .insert("origin", "http://evil.test".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut req = request("POST", "/direct-admin/v1/tokens", KEY, r#"{"name":"test"}"#);
    req.headers_mut()
        .insert("sec-fetch-site", "cross-site".parse().unwrap());
    assert_eq!(
        app.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.oneshot(request("GET", "/health", KEY, ""))
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn create_list_edit_revoke_and_no_cache() {
    let app = app();
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            "/direct-admin/v1/tokens",
            KEY,
            r#"{"name":"脚本","rate_per_minute":2}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let data: serde_json::Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let token = data["data"]["token"].as_str().unwrap();
    let id = data["data"]["record"]["id"].as_str().unwrap();
    let response = app
        .clone()
        .oneshot(request("GET", "/direct-admin/v1/tokens", KEY, ""))
        .await
        .unwrap();
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(!body.contains(token));
    assert!(!body.contains("secret_hash"));
    let response = app
        .clone()
        .oneshot(request(
            "POST",
            &format!("/direct-admin/v1/tokens/{id}/revoke"),
            KEY,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        app.oneshot(request(
            "PATCH",
            &format!("/direct-admin/v1/tokens/{id}"),
            KEY,
            r#"{"enabled":true}"#
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::FORBIDDEN
    );
}
#[tokio::test]
async fn rejects_bad_json_and_body_size() {
    let app = app();
    let response = app
        .clone()
        .oneshot(request("POST", "/direct-admin/v1/tokens", KEY, "not json"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()["content-type"], "application/json");
    let huge = format!(r#"{{"name":"{}"}}"#, "x".repeat(17000));
    assert_eq!(
        app.oneshot(request("POST", "/direct-admin/v1/tokens", KEY, &huge))
            .await
            .unwrap()
            .status(),
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
#[test]
fn configuration_fails_closed() {
    assert!(Config::new("".into(), vec!["http://example.test".into()]).is_err());
    assert!(Config::new(KEY.into(), vec![]).is_err());
    assert!(Config::new(KEY.into(), vec!["*".into()]).is_err());
}
