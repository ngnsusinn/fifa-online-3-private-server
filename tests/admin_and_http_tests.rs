use fo3_server::admin::create_admin_router;
use fo3_server::http::routes::create_http_router;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use serde_json::{json, Value};
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
    let html_bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(html_bytes.to_vec()).unwrap();
    assert!(html.contains("Quản trị FO3"));
    assert!(html.contains("Tổng quan"));
    assert!(html.contains("Cầu thủ CLB"));

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

    // 7. Mutation API with invalid value -> error in Vietnamese
    let req_ep_invalid = Request::builder()
        .method(Method::POST)
        .uri("/api/ep")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"set": -100}"#))
        .unwrap();

    let resp_ep_invalid = app.clone().oneshot(req_ep_invalid).await.unwrap();
    assert_eq!(resp_ep_invalid.status(), StatusCode::OK);
    let body_bytes_err = to_bytes(resp_ep_invalid.into_body(), usize::MAX).await.unwrap();
    let err_res: Value = serde_json::from_slice(&body_bytes_err).unwrap();
    assert!(err_res["error"].as_str().unwrap().contains("EP phải từ"));
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

    // 6. Test English locale and SG region
    std::env::set_var("FO3_LANGUAGE", "eng");
    let req_locale = Request::builder()
        .uri("/info/server/locale")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_locale = app.clone().oneshot(req_locale).await.unwrap();
    assert_eq!(resp_locale.status(), StatusCode::OK);
    let bytes = to_bytes(resp_locale.into_body(), usize::MAX).await.unwrap();
    let val_locale: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val_locale["language"], "eng");
    assert_eq!(val_locale["result"], true);

    let req_conf_eng = Request::builder()
        .uri("/info/server/config")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_conf_eng = app.clone().oneshot(req_conf_eng).await.unwrap();
    assert_eq!(resp_conf_eng.status(), StatusCode::OK);
    let bytes = to_bytes(resp_conf_eng.into_body(), usize::MAX).await.unwrap();
    let val_conf: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val_conf["region"], "SG");

    // 7. Test inweburl handler
    let req_inweb = Request::builder()
        .uri("/game/inweburl/notice")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_inweb = app.clone().oneshot(req_inweb).await.unwrap();
    assert_eq!(resp_inweb.status(), StatusCode::OK);
    let bytes = to_bytes(resp_inweb.into_body(), usize::MAX).await.unwrap();
    let val_inweb: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(val_inweb["url"], "about:blank");
    assert_eq!(val_inweb["result"], true);

    // 8. Test CDN language_eng.big asset serving
    let req_cdn_eng = Request::builder()
        .uri("/cdn/externalAssets/language_eng.big")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_cdn_eng = app.clone().oneshot(req_cdn_eng).await.unwrap();
    assert_eq!(resp_cdn_eng.status(), StatusCode::OK);
    let bytes_cdn = to_bytes(resp_cdn_eng.into_body(), usize::MAX).await.unwrap();
    assert!(!bytes_cdn.is_empty());

    // 9. Test /account returns options
    let req_account = Request::builder()
        .uri("/account")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_account = app.clone().oneshot(req_account).await.unwrap();
    assert_eq!(resp_account.status(), StatusCode::OK);
    let bytes_acc = to_bytes(resp_account.into_body(), usize::MAX).await.unwrap();
    let val_acc: Value = serde_json::from_slice(&bytes_acc).unwrap();
    assert_eq!(val_acc["result"], true);
    assert!(val_acc.get("option").is_some());
    assert!(val_acc["option"].get("soundOption").is_some());
    assert!(val_acc["option"].get("controlOption").is_some());
    assert!(val_acc.get("options").is_some());

    // 10. Test /coaches returns coachlist with summary, teamvalue, levelinfo
    let req_coaches = Request::builder()
        .uri("/coaches")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_coaches = app.clone().oneshot(req_coaches).await.unwrap();
    assert_eq!(resp_coaches.status(), StatusCode::OK);
    let bytes_c = to_bytes(resp_coaches.into_body(), usize::MAX).await.unwrap();
    let val_c: Value = serde_json::from_slice(&bytes_c).unwrap();
    assert!(val_c.get("coachlist").is_some());
    assert!(val_c.get("coaches").is_some());
    let coachlist = val_c["coachlist"].as_array().unwrap();
    if !coachlist.is_empty() {
        assert!(coachlist[0].get("summary").is_some());
        assert!(coachlist[0].get("teamvalue").is_some());
        assert!(coachlist[0].get("levelinfo").is_some());
        assert_eq!(coachlist[0]["state"], "active");
    }

    // 11. Test /coach/me returns root fields
    let req_coach_me = Request::builder()
        .uri("/coach/me")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_coach_me = app.clone().oneshot(req_coach_me).await.unwrap();
    assert_eq!(resp_coach_me.status(), StatusCode::OK);
    let bytes_me = to_bytes(resp_coach_me.into_body(), usize::MAX).await.unwrap();
    let val_me: Value = serde_json::from_slice(&bytes_me).unwrap();
    assert_eq!(val_me["result"], true);
    assert!(val_me.get("money").is_some());
    assert_eq!(val_me["money"]["mpdata"]["until"], "2030-01-01T00:00:00.000Z");
    assert!(val_me.get("limits").is_some());
    assert_eq!(val_me["limits"]["firstTeam"], 36);

    // 12. Test /coach/0/default returns resources with body.id
    let req_default = Request::builder()
        .method(Method::POST)
        .uri("/coach/0/default")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"_method":"PUT"}"#))
        .unwrap();
    let resp_default = app.clone().oneshot(req_default).await.unwrap();
    assert_eq!(resp_default.status(), StatusCode::OK);
    let bytes_def = to_bytes(resp_default.into_body(), usize::MAX).await.unwrap();
    let val_def: Value = serde_json::from_slice(&bytes_def).unwrap();
    assert_eq!(val_def["result"], true);
    assert!(val_def.get("resources").is_some());
    let res = val_def["resources"].as_array().unwrap();
    assert!(!res.is_empty());
    assert_eq!(res[0]["uri"], "/coach/me");
    assert!(res[0]["body"].get("id").is_some());

    // 13. Test /events returns valid condition and timeset
    let req_events = Request::builder()
        .uri("/events")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_events = app.clone().oneshot(req_events).await.unwrap();
    assert_eq!(resp_events.status(), StatusCode::OK);
    let bytes_ev = to_bytes(resp_events.into_body(), usize::MAX).await.unwrap();
    let val_ev: Value = serde_json::from_slice(&bytes_ev).unwrap();
    let ev_arr = val_ev["events"].as_array().unwrap();
    assert!(!ev_arr.is_empty());
    assert!(ev_arr[0].get("condition").is_some());
    assert_eq!(ev_arr[0]["condition"]["timeset"]["startdate"], "20140101");

    // 14. Test /shop/list returns a JSON Array (not an object) to prevent ActionScript Error #1034
    let req_shop = Request::builder()
        .uri("/shop/list")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_shop = app.clone().oneshot(req_shop).await.unwrap();
    assert_eq!(resp_shop.status(), StatusCode::OK);
    let bytes_shop = to_bytes(resp_shop.into_body(), usize::MAX).await.unwrap();
    let val_shop: Value = serde_json::from_slice(&bytes_shop).unwrap();
    assert!(val_shop.is_array());

    // 15. Test /coach/me/squad correctly handles PUT with nested squad and returns resources
    let starter_ids = [
        88048940u64, 88203747, 88165229, 88192227, 88207993, 88176635, 88199503, 88186561, 88184941, 88178509, 88193301,
    ];
    let mut test_positions = Vec::new();
    for (i, &spid) in starter_ids.iter().enumerate() {
        test_positions.push(json!({
            "id": spid.to_string(),
            "pos": i,
            "spid": spid,
            "starting": [1, 1, 0.5, 0.02]
        }));
    }
    let squad_payload = json!({
        "_method": "PUT",
        "captain": "88184941",
        "kickers": ["88199503", "88186561", "88184941", "88178509", "88193301"],
        "squad": {
            "captain": "88184941",
            "formation": 1,
            "kickers": ["88199503", "88186561", "88184941", "88178509", "88193301"],
            "positions": test_positions
        }
    });
    let req_squad = Request::builder()
        .method(Method::POST)
        .uri("/coach/me/squad")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(squad_payload.to_string()))
        .unwrap();
    let resp_squad = app.clone().oneshot(req_squad).await.unwrap();
    assert_eq!(resp_squad.status(), StatusCode::OK);
    let bytes_sq = to_bytes(resp_squad.into_body(), usize::MAX).await.unwrap();
    let val_sq: Value = serde_json::from_slice(&bytes_sq).unwrap();
    assert_eq!(val_sq["result"], true);
    assert!(val_sq.get("resources").is_some());
    let sq_res = val_sq["resources"].as_array().unwrap();
    assert_eq!(sq_res[0]["uri"], "/coach/me");
    let sq_body = &sq_res[0]["body"];
    assert_eq!(sq_body["team"]["captain"], "88184941");
    let saved_pos = sq_body["team"]["squad"]["positions"].as_array().unwrap();
    assert_eq!(saved_pos.len(), 11);
    assert_eq!(saved_pos[0]["id"], "88048940");
}
