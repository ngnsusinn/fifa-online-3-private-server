use fo3_server::admin::create_admin_router;
use fo3_server::http::routes::create_http_router;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use serde_json::Value;
use std::net::SocketAddr;
use tower::ServiceExt;

#[tokio::test]
async fn test_admin_page_and_security() {
    let app = create_admin_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET / with valid Host
    let req = Request::builder()
        .uri("/")
        .header(header::HOST, "127.0.0.1:8088")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let ctype = resp.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(ctype.contains("text/html"));

    // 2. Forbidden Host
    let req_bad_host = Request::builder()
        .uri("/")
        .header(header::HOST, "evil.com")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_bad_host = app.clone().oneshot(req_bad_host).await.unwrap();
    assert_eq!(resp_bad_host.status(), StatusCode::FORBIDDEN);

    // 3. Forbidden Remote IP
    let ext_addr: SocketAddr = "192.168.1.50:12345".parse().unwrap();
    let req_ext = Request::builder()
        .uri("/")
        .header(header::HOST, "127.0.0.1")
        .extension(axum::extract::ConnectInfo(ext_addr))
        .body(Body::empty())
        .unwrap();

    let resp_ext = app.clone().oneshot(req_ext).await.unwrap();
    assert_eq!(resp_ext.status(), StatusCode::FORBIDDEN);

    // 4. Read-only API: /api/summary
    let req_summary = Request::builder()
        .uri("/api/summary")
        .header(header::HOST, "localhost:8088")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_summary = app.clone().oneshot(req_summary).await.unwrap();
    assert_eq!(resp_summary.status(), StatusCode::OK);
    let body_bytes = to_bytes(resp_summary.into_body(), usize::MAX).await.unwrap();
    let summary: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(summary.get("coach").is_some());
    assert!(summary.get("vip_until").is_some());

    // 5. Mutation API without X-FO3-Admin header: /api/ep -> 403
    let req_ep_bad = Request::builder()
        .method(Method::POST)
        .uri("/api/ep")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"set": 5000000}"#))
        .unwrap();

    let resp_ep_bad = app.clone().oneshot(req_ep_bad).await.unwrap();
    assert_eq!(resp_ep_bad.status(), StatusCode::FORBIDDEN);

    // 6. Mutation API with X-FO3-Admin header -> 200 OK
    let req_ep_good = Request::builder()
        .method(Method::POST)
        .uri("/api/ep")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"set": 5000000}"#))
        .unwrap();

    let resp_ep_good = app.clone().oneshot(req_ep_good).await.unwrap();
    assert_eq!(resp_ep_good.status(), StatusCode::OK);
    let body_bytes = to_bytes(resp_ep_good.into_body(), usize::MAX).await.unwrap();
    let ep_res: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(ep_res["ep"], 5000000);
}

#[tokio::test]
async fn test_http_router_and_fallbacks() {
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. Exact endpoint: /info/server/config
    let req = Request::builder()
        .uri("/info/server/config")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let val: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val["status"], "ok");
    assert_eq!(val["redirector"]["port"], 28000);

    // 2. Exact endpoint: /info/server/time
    let req_time = Request::builder()
        .uri("/info/server/time")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_time = app.clone().oneshot(req_time).await.unwrap();
    assert_eq!(resp_time.status(), StatusCode::OK);
    let bytes = to_bytes(resp_time.into_body(), usize::MAX).await.unwrap();
    let val_time: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(val_time.get("timestamp").is_some());

    // 3. Fallback to responses/static_roster.json for /static/roster
    let req_roster = Request::builder()
        .uri("/static/roster")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_roster = app.clone().oneshot(req_roster).await.unwrap();
    assert_eq!(resp_roster.status(), StatusCode::OK);
    let bytes = to_bytes(resp_roster.into_body(), usize::MAX).await.unwrap();
    let val_roster: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(val_roster.is_object() || val_roster.is_array());

    // 4. Unknown GET returns {}
    let req_unknown_get = Request::builder()
        .method(Method::GET)
        .uri("/nonexistent_endpoint_xyz")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_unk_get = app.clone().oneshot(req_unknown_get).await.unwrap();
    assert_eq!(resp_unk_get.status(), StatusCode::OK);
    let bytes = to_bytes(resp_unk_get.into_body(), usize::MAX).await.unwrap();
    let val_unk: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val_unk, serde_json::json!({}));

    // 5. Unknown POST returns {"result": true}
    let req_unknown_post = Request::builder()
        .method(Method::POST)
        .uri("/nonexistent_endpoint_xyz")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_unk_post = app.clone().oneshot(req_unknown_post).await.unwrap();
    assert_eq!(resp_unk_post.status(), StatusCode::OK);
    let bytes = to_bytes(resp_unk_post.into_body(), usize::MAX).await.unwrap();
    let val_post: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val_post, serde_json::json!({"result": true}));
}
