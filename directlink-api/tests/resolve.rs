use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use baidupcs_directlink_api::{
    http::{router, Config},
    store::{CreateToken, Store},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn public_resolver_requires_bearer_not_admin_or_query_token() {
    let store = Store::open_memory().unwrap();
    let app = router(
        store,
        Config::new("a".repeat(40), vec!["http://example.test".into()]).unwrap(),
    );
    for path in [
        "/direct-api/v1/resolve",
        "/direct-api/v1/shares/preview?token=dl_fake",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(path)
                    .header("x-directlink-admin-key", "a".repeat(40))
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
#[tokio::test]
async fn authenticated_invalid_url_is_rejected_before_upstream() {
    let store = Store::open_memory().unwrap();
    let token = store
        .create(
            CreateToken {
                max_uses: None,
                name: "test".into(),
                note: "".into(),
                expires_at: None,
                rate_per_minute: 10,
            },
            1,
        )
        .unwrap()
        .token;
    let app = router(
        store,
        Config::new("a".repeat(40), vec!["http://example.test".into()]).unwrap(),
    );
    for url in [
        "https://evil.test/s/abc",
        "http://127.0.0.1/",
        "https://pan.baidu.com.evil.test/s/abc",
        "https://user@pan.baidu.com/s/abc",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/direct-api/v1/resolve")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::json!({"share_url":url}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(!String::from_utf8_lossy(&body).contains(url));
    }
}
