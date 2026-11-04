use fo3_server::admin::create_admin_router;
use fo3_server::http::routes::create_http_router;
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tower::ServiceExt;

static SERIAL_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_admin_page_and_security() {
    let _test_guard = SERIAL_TEST.lock().await;
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

    // 8. Mutation API: /api/ep with add -> sends to Bag as gift
    let req_ep_add = Request::builder()
        .method(Method::POST)
        .uri("/api/ep")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"add": 1000000}"#))
        .unwrap();
    let resp_ep_add = app.clone().oneshot(req_ep_add).await.unwrap();
    assert_eq!(resp_ep_add.status(), StatusCode::OK);
    let bytes_add = to_bytes(resp_ep_add.into_body(), usize::MAX).await.unwrap();
    let val_add: Value = serde_json::from_slice(&bytes_add).unwrap();
    assert_eq!(val_add["ok"], true);

    // 9. Mutation API: /api/give_pack -> sends pack into Bag as gift
    let req_give_pack = Request::builder()
        .method(Method::POST)
        .uri("/api/give_pack")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"itemid": 28001, "count": 2}"#))
        .unwrap();
    let resp_give_pack = app.clone().oneshot(req_give_pack).await.unwrap();
    assert_eq!(resp_give_pack.status(), StatusCode::OK);
    let bytes_gp = to_bytes(resp_give_pack.into_body(), usize::MAX).await.unwrap();
    let val_gp: Value = serde_json::from_slice(&bytes_gp).unwrap();
    assert_eq!(val_gp["ok"], true);
}

#[tokio::test]
async fn test_http_router_and_fallbacks() {
    let _test_guard = SERIAL_TEST.lock().await;
    {
        let mut st = fo3_server::state::load_state();
        let slot = fo3_server::state::default_slot(&st);
        if let Some(players) = st.get_mut("teams").and_then(|t| t.get_mut(&slot.to_string())).and_then(|t| t.get_mut("players")).and_then(|p| p.as_array_mut()) {
            if players.len() > 50 {
                players.truncate(36);
                let _ = fo3_server::state::save_state(&st);
            }
        }
    }
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
    assert_eq!(val_me["team"]["staffs"], json!([]));
    assert_eq!(val_me["team"]["staffroles"], json!([]));
    assert_eq!(val_me["team"]["staffroleidx"], 0);
    assert_eq!(val_me["team"]["maxstaff"], 100);
    assert_eq!(val_me["team"]["staffsmc"], json!({}));

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

    // 16. Test /bag returns a JSON Array with valid bag entry format
    let req_bag = Request::builder()
        .uri("/bag")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_bag = app.clone().oneshot(req_bag).await.unwrap();
    assert_eq!(resp_bag.status(), StatusCode::OK);
    let bytes_bag = to_bytes(resp_bag.into_body(), usize::MAX).await.unwrap();
    let val_bag: Value = serde_json::from_slice(&bytes_bag).unwrap();
    assert!(val_bag.is_array());

    // 17. Test /static/cache/item returns definitions
    let req_item_cache = Request::builder()
        .method(Method::POST)
        .uri("/static/cache/item")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"itemids": [1, 801, 28001]}"#))
        .unwrap();
    let resp_item_cache = app.clone().oneshot(req_item_cache).await.unwrap();
    assert_eq!(resp_item_cache.status(), StatusCode::OK);
    let bytes_ic = to_bytes(resp_item_cache.into_body(), usize::MAX).await.unwrap();
    let val_ic: Value = serde_json::from_slice(&bytes_ic).unwrap();
    assert!(val_ic.get("1").is_some());
    assert_eq!(val_ic["1"]["name"], "EP");
    assert!(val_ic.get("801").is_some());
    assert!(val_ic.get("28001").is_some());

    // 18. Test admin sending gift -> /bag shows it -> /bag/<id>/unpack claims it
    let admin_app = create_admin_router();
    let req_admin_gift = Request::builder()
        .method(Method::POST)
        .uri("/api/bag_send")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(
            r#"{"title": "Test Gift EP", "ep": 50000, "pack": 28001, "count": 1}"#,
        ))
        .unwrap();
    let resp_admin_gift = admin_app.clone().oneshot(req_admin_gift).await.unwrap();
    assert_eq!(resp_admin_gift.status(), StatusCode::OK);
    let bytes_ag = to_bytes(resp_admin_gift.into_body(), usize::MAX).await.unwrap();
    let val_ag: Value = serde_json::from_slice(&bytes_ag).unwrap();
    let gift_id = val_ag["id"].as_str().unwrap();

    // Verify it appears in /bag
    let req_bag2 = Request::builder()
        .uri("/bag")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_bag2 = app.clone().oneshot(req_bag2).await.unwrap();
    let bytes_bag2 = to_bytes(resp_bag2.into_body(), usize::MAX).await.unwrap();
    let val_bag2: Value = serde_json::from_slice(&bytes_bag2).unwrap();
    let bag_arr = val_bag2.as_array().unwrap();
    assert!(bag_arr.iter().any(|e| e["_id"] == gift_id));

    // Claim via /bag/<id>/unpack
    let req_unpack = Request::builder()
        .method(Method::POST)
        .uri(format!("/bag/{}/unpack", gift_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_unpack = app.clone().oneshot(req_unpack).await.unwrap();
    assert_eq!(resp_unpack.status(), StatusCode::OK);
    let bytes_up = to_bytes(resp_unpack.into_body(), usize::MAX).await.unwrap();
    let val_up: Value = serde_json::from_slice(&bytes_up).unwrap();
    assert_eq!(val_up["result"], true);
    assert!(val_up.get("resources").is_some());
    let up_res = val_up["resources"].as_array().unwrap();
    assert_eq!(up_res[0]["uri"], "/coach/me");
    assert_eq!(up_res[1]["uri"], "/bag");

    // 19. Test /item/depot returns array
    let req_depot = Request::builder()
        .uri("/item/depot")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_depot = app.clone().oneshot(req_depot).await.unwrap();
    assert_eq!(resp_depot.status(), StatusCode::OK);
    let bytes_depot = to_bytes(resp_depot.into_body(), usize::MAX).await.unwrap();
    let val_depot: Value = serde_json::from_slice(&bytes_depot).unwrap();
    assert!(val_depot.is_array());

    // 20. Test /item/<id>/use opens a pack
    let coach_me_body = &up_res[0]["body"];
    let items_in_coach = coach_me_body["items"].as_array().unwrap();
    let pack_stack = items_in_coach
        .iter()
        .find(|i| i["itemid"] == 28001)
        .expect("Pack 28001 should be in items after unpack");
    let pack_item_id = pack_stack["_id"].as_str().unwrap();

    let req_use_pack = Request::builder()
        .method(Method::POST)
        .uri(format!("/item/{}/use", pack_item_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_use_pack = app.clone().oneshot(req_use_pack).await.unwrap();
    assert_eq!(resp_use_pack.status(), StatusCode::OK);
    let bytes_upk = to_bytes(resp_use_pack.into_body(), usize::MAX).await.unwrap();
    let val_upk: Value = serde_json::from_slice(&bytes_upk).unwrap();
    assert_eq!(val_upk["result"], true);
    assert!(val_upk.get("player").is_some());
    assert!(val_upk["player"].get("spid").is_some());
    assert!(val_upk.get("resources").is_some());
}

#[tokio::test]
async fn test_staff_system_and_endpoints() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. Check get_default_team contains staff fields
    let team = fo3_server::state::get_default_team(1);
    assert_eq!(team["staffs"], json!([]));
    assert_eq!(team["staffroles"], json!([]));
    assert_eq!(team["staffroleidx"], 0);
    assert_eq!(team["maxstaff"], 100);
    assert_eq!(team["staffsmc"], json!({}));

    // 2. GET /staff/status/expand
    let req_expand = Request::builder()
        .uri("/staff/status/expand")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_expand = app.clone().oneshot(req_expand).await.unwrap();
    assert_eq!(resp_expand.status(), StatusCode::OK);
    let bytes_exp = to_bytes(resp_expand.into_body(), usize::MAX).await.unwrap();
    let val_exp: Value = serde_json::from_slice(&bytes_exp).unwrap();
    assert_eq!(val_exp["result"], true);
    assert_eq!(val_exp["expandable"], false);
    assert_eq!(val_exp["nextmaxcount"], 100);
    assert_eq!(val_exp["nextfee"], 0);

    // 3. GET /staff/recruit returns JSON Array []
    let req_recruit = Request::builder()
        .uri("/staff/recruit")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_recruit = app.clone().oneshot(req_recruit).await.unwrap();
    assert_eq!(resp_recruit.status(), StatusCode::OK);
    let bytes_rec = to_bytes(resp_recruit.into_body(), usize::MAX).await.unwrap();
    let val_rec: Value = serde_json::from_slice(&bytes_rec).unwrap();
    assert!(val_rec.is_array(), "Response must be a JSON Array for Flash client");
    assert_eq!(val_rec, json!([]));

    // 4. GET /static/db/STAFF returns config slot structure
    let req_staff_db = Request::builder()
        .uri("/static/db/STAFF")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_staff_db = app.clone().oneshot(req_staff_db).await.unwrap();
    assert_eq!(resp_staff_db.status(), StatusCode::OK);
    let bytes_db = to_bytes(resp_staff_db.into_body(), usize::MAX).await.unwrap();
    let val_db: Value = serde_json::from_slice(&bytes_db).unwrap();
    assert_eq!(val_db["config"]["max_staff_open_count"], 3);
    assert!(val_db["slots"]["1"].is_array());

    // 5. GET /coach/me has team staff fields
    let req_me = Request::builder()
        .uri("/coach/me")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_me = app.clone().oneshot(req_me).await.unwrap();
    assert_eq!(resp_me.status(), StatusCode::OK);
    let bytes_me = to_bytes(resp_me.into_body(), usize::MAX).await.unwrap();
    let val_me: Value = serde_json::from_slice(&bytes_me).unwrap();
    assert_eq!(val_me["team"]["staffs"], json!([]));
    assert_eq!(val_me["team"]["staffroles"], json!([]));
    assert_eq!(val_me["team"]["staffroleidx"], 0);
    assert_eq!(val_me["team"]["maxstaff"], 100);
    assert_eq!(val_me["team"]["staffsmc"], json!({}));
}

#[tokio::test]
async fn test_shop_endpoints_and_purchase() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /shop/list
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
    let shop_arr = val_shop.as_array().expect("Shop list must be array");
    assert!(!shop_arr.is_empty(), "Shop list must not be empty");

    let first = &shop_arr[0];
    assert!(first.get("_id").is_some());
    assert!(first.get("shopid").is_some());
    assert!(first.get("shopcategory").is_some());
    assert_eq!(first.get("forceenable").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(first.get("visible").and_then(|v| v.as_bool()), Some(true));
    assert!(first.get("product").is_some());

    // 2. GET /shop/list/STSHOP
    let req_st = Request::builder()
        .uri("/shop/list/STSHOP")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_st = app.clone().oneshot(req_st).await.unwrap();
    assert_eq!(resp_st.status(), StatusCode::OK);
    let bytes_st = to_bytes(resp_st.into_body(), usize::MAX).await.unwrap();
    let val_st: Value = serde_json::from_slice(&bytes_st).unwrap();
    let st_arr = val_st.as_array().expect("STSHOP must be array");
    assert_eq!(st_arr.len(), 4);
    assert_eq!(st_arr[0]["stid"], st_arr[0]["shopid"]);

    // 3. GET /shop/list/GPSHOP
    let req_gp = Request::builder()
        .uri("/shop/list/GPSHOP")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_gp = app.clone().oneshot(req_gp).await.unwrap();
    assert_eq!(resp_gp.status(), StatusCode::OK);
    let bytes_gp = to_bytes(resp_gp.into_body(), usize::MAX).await.unwrap();
    let val_gp: Value = serde_json::from_slice(&bytes_gp).unwrap();
    let gp_arr = val_gp.as_array().expect("GPSHOP must be array");
    assert_eq!(gp_arr.len(), 5);
    assert_eq!(gp_arr[0]["shopcategory"], "GPSHOP");

    // 4. GET /shop/remained/cash
    let req_cash = Request::builder()
        .uri("/shop/remained/cash")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_cash = app.clone().oneshot(req_cash).await.unwrap();
    assert_eq!(resp_cash.status(), StatusCode::OK);
    let bytes_cash = to_bytes(resp_cash.into_body(), usize::MAX).await.unwrap();
    let val_cash: Value = serde_json::from_slice(&bytes_cash).unwrap();
    assert_eq!(val_cash["remainedcash"], 0);

    // 5. GET /specifictarget
    let req_tgt = Request::builder()
        .uri("/specifictarget")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_tgt = app.clone().oneshot(req_tgt).await.unwrap();
    assert_eq!(resp_tgt.status(), StatusCode::OK);
    let bytes_tgt = to_bytes(resp_tgt.into_body(), usize::MAX).await.unwrap();
    let val_tgt: Value = serde_json::from_slice(&bytes_tgt).unwrap();
    assert_eq!(val_tgt["issell"], true);
    assert_eq!(val_tgt["targets"], json!([]));

    // 6. GET /monthlypack
    let req_mp = Request::builder()
        .uri("/monthlypack")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_mp = app.clone().oneshot(req_mp).await.unwrap();
    assert_eq!(resp_mp.status(), StatusCode::OK);
    let bytes_mp = to_bytes(resp_mp.into_body(), usize::MAX).await.unwrap();
    let val_mp: Value = serde_json::from_slice(&bytes_mp).unwrap();
    assert!(val_mp.is_array());

    // 7. POST /shop/purchase
    let buy_payload = json!({
        "shopid": 28001,
        "productid": 28001,
        "bunch": 0
    });
    let req_buy = Request::builder()
        .method(Method::POST)
        .uri("/shop/purchase")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(buy_payload.to_string()))
        .unwrap();
    let resp_buy = app.clone().oneshot(req_buy).await.unwrap();
    assert_eq!(resp_buy.status(), StatusCode::OK);
    let bytes_buy = to_bytes(resp_buy.into_body(), usize::MAX).await.unwrap();
    let val_buy: Value = serde_json::from_slice(&bytes_buy).unwrap();
    assert_eq!(val_buy["result"], true);
    let resources = val_buy["resources"].as_array().expect("resources must be array");
    assert_eq!(resources[0]["uri"], "/coach/me");

    // 8. POST /shop/purchase invalid product
    let buy_invalid = json!({
        "shopid": 99999999,
        "productid": 99999999,
        "bunch": 0
    });
    let req_invalid = Request::builder()
        .method(Method::POST)
        .uri("/shop/purchase")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(buy_invalid.to_string()))
        .unwrap();
    let resp_invalid = app.clone().oneshot(req_invalid).await.unwrap();
    assert_eq!(resp_invalid.status(), StatusCode::OK);
    let bytes_invalid = to_bytes(resp_invalid.into_body(), usize::MAX).await.unwrap();
    let val_invalid: Value = serde_json::from_slice(&bytes_invalid).unwrap();
    assert_eq!(val_invalid["result"], false);
    assert_eq!(val_invalid["reason"], "NOT_EXIST_PRODUCT");
}

#[tokio::test]
async fn test_miniface_cdn_and_fallback() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. Existing base miniface (p179846.png) returns 200 OK
    let req_base = Request::builder()
        .uri("/cdn/externalAssets/players/p179846.png")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_base = app.clone().oneshot(req_base).await.unwrap();
    assert_eq!(resp_base.status(), StatusCode::OK);
    let ctype_base = resp_base.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert_eq!(ctype_base, "image/png");
    let bytes_base = to_bytes(resp_base.into_body(), usize::MAX).await.unwrap();
    assert!(bytes_base.starts_with(b"\x89PNG"));

    // 2. Season miniface fallback to base player face (p67179846.png -> p179846.png)
    let req_season = Request::builder()
        .uri("/cdn/externalAssets/players/p67179846.png")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_season = app.clone().oneshot(req_season).await.unwrap();
    assert_eq!(resp_season.status(), StatusCode::OK);
    let bytes_season = to_bytes(resp_season.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes_season, bytes_base);

    // 3. Unknown player face falls back to default 128x128 PNG (not 404!)
    fo3_server::http::miniface::mark_failed_pid(999997);
    fo3_server::http::miniface::mark_failed_pid(888999997);
    let req_missing = Request::builder()
        .uri("/cdn/externalAssets/players/p888999997.png")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_missing = app.clone().oneshot(req_missing).await.unwrap();
    assert_eq!(resp_missing.status(), StatusCode::OK);
    let ctype_missing = resp_missing.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert_eq!(ctype_missing, "image/png");
    let bytes_missing = to_bytes(resp_missing.into_body(), usize::MAX).await.unwrap();
    assert_eq!(bytes_missing, fo3_server::http::miniface::DEFAULT_PLAYER_PNG);

    // 4. Non-player files like _timestamp_nonexistent MUST STILL 404 if not on disk
    let req_ts = Request::builder()
        .uri("/cdn/externalAssets/players/_timestamp_nonexistent")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_ts = app.clone().oneshot(req_ts).await.unwrap();
    assert_eq!(resp_ts.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_trade_system_and_flow() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /trade3/staticvalues
    let req_static = Request::builder()
        .uri("/trade3/staticvalues")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_static = app.clone().oneshot(req_static).await.unwrap();
    assert_eq!(resp_static.status(), StatusCode::OK);
    let bytes_static = to_bytes(resp_static.into_body(), usize::MAX).await.unwrap();
    let val_static: Value = serde_json::from_slice(&bytes_static).unwrap();
    assert!(val_static.get("playertrade3_limit").is_some());
    assert!(val_static.get("vvip_playertrade3_limit").is_some());
    assert_eq!(val_static["playertrade3_normal"]["2"], 1);
    assert_eq!(val_static["playertrade3_normal"]["4"], 2);
    assert_eq!(val_static["playertrade3_normal"]["7"], 3);
    assert_eq!(val_static["playertrade3_effect_price"], json!([10000000, 3000000, 1000000]));

    // 2. POST /trade3/info
    let req_t3_info = Request::builder()
        .method(Method::POST)
        .uri("/trade3/info")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_t3_info = app.clone().oneshot(req_t3_info).await.unwrap();
    assert_eq!(resp_t3_info.status(), StatusCode::OK);
    let bytes_t3_info = to_bytes(resp_t3_info.into_body(), usize::MAX).await.unwrap();
    let val_t3_info: Value = serde_json::from_slice(&bytes_t3_info).unwrap();
    assert_eq!(val_t3_info["result"], true);
    assert!(val_t3_info.get("resources").is_some());
    let t3_res = val_t3_info["resources"].as_array().unwrap();
    assert_eq!(t3_res[0]["uri"], "/coach/me");
    let scouters = t3_res[0]["body"]["scouters"].as_array().unwrap();
    assert_eq!(scouters.len(), 5);
    assert!(scouters.iter().any(|s| s["key"] == "default_B01"));
    assert!(scouters.iter().any(|s| s["key"] == "igr"));
    assert!(scouters.iter().any(|s| s["key"] == "vip"));
    assert!(scouters.iter().any(|s| s["key"] == "event"));
    assert!(scouters.iter().any(|s| s["key"] == "vvip"));

    // 3. GET /static/db/TRADE (trade2 table)
    let req_t2_db = Request::builder()
        .uri("/static/db/TRADE")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_t2_db = app.clone().oneshot(req_t2_db).await.unwrap();
    assert_eq!(resp_t2_db.status(), StatusCode::OK);
    let bytes_t2_db = to_bytes(resp_t2_db.into_body(), usize::MAX).await.unwrap();
    let val_t2_db: Value = serde_json::from_slice(&bytes_t2_db).unwrap();
    assert!(val_t2_db.get("bonusplayergroup").is_some());
    assert!(val_t2_db.get("playertrade2_bonus").is_some());
    assert!(val_t2_db.get("legendgroup").is_some());
    assert!(val_t2_db.get("tp").is_some());

    // 4. GET /static/cache/TRADE_SPECIALPLAYERS
    let req_special = Request::builder()
        .uri("/static/cache/TRADE_SPECIALPLAYERS")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_special = app.clone().oneshot(req_special).await.unwrap();
    assert_eq!(resp_special.status(), StatusCode::OK);
    let bytes_special = to_bytes(resp_special.into_body(), usize::MAX).await.unwrap();
    let val_special: Value = serde_json::from_slice(&bytes_special).unwrap();
    assert!(val_special.is_object());

    // 5. GET /players/trade2/info
    let req_t2_info = Request::builder()
        .uri("/players/trade2/info")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_t2_info = app.clone().oneshot(req_t2_info).await.unwrap();
    assert_eq!(resp_t2_info.status(), StatusCode::OK);
    let bytes_t2_info = to_bytes(resp_t2_info.into_body(), usize::MAX).await.unwrap();
    let val_t2_info: Value = serde_json::from_slice(&bytes_t2_info).unwrap();
    assert!(val_t2_info.is_object());
    assert!(val_t2_info.get("1000000").is_some());

    // 6. GET /account/trade2/usage
    let req_usage = Request::builder()
        .uri("/account/trade2/usage")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_usage = app.clone().oneshot(req_usage).await.unwrap();
    assert_eq!(resp_usage.status(), StatusCode::OK);
    let bytes_usage = to_bytes(resp_usage.into_body(), usize::MAX).await.unwrap();
    let val_usage: Value = serde_json::from_slice(&bytes_usage).unwrap();
    assert_eq!(val_usage["result"], true);
    assert_eq!(val_usage["usedcount"], 0);

    // 7. GET /static/db/PLAYERTRADE
    let req_pt = Request::builder()
        .uri("/static/db/PLAYERTRADE")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_pt = app.clone().oneshot(req_pt).await.unwrap();
    assert_eq!(resp_pt.status(), StatusCode::OK);
    let bytes_pt = to_bytes(resp_pt.into_body(), usize::MAX).await.unwrap();
    let val_pt: Value = serde_json::from_slice(&bytes_pt).unwrap();
    assert!(val_pt.get("playergrade").is_some());
    assert!(val_pt["playergrade"].get("2").is_some());

    // 8. Prepare state with 2 extra non-squad players for Trade 3
    let (p1_id, p1_spid, p2_id, p2_spid) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let slot = fo3_server::state::default_slot(&st);
        let team = st.get_mut("teams").unwrap().get_mut(&slot.to_string()).unwrap();
        let p1 = fo3_server::state::add_player(team, 101000001, 1);
        let p2 = fo3_server::state::add_player(team, 101000002, 1);
        let _ = fo3_server::state::save_state(&st);
        (
            p1["_id"].as_str().unwrap().to_string(),
            p1["spid"].as_u64().unwrap(),
            p2["_id"].as_str().unwrap().to_string(),
            p2["spid"].as_u64().unwrap(),
        )
    };

    // Move players to secondteam (Tim Cadangan)
    let req_mv1 = Request::builder()
        .method(Method::POST)
        .uri(format!("/player/{}/move/secondteam", p1_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_mv1 = app.clone().oneshot(req_mv1).await.unwrap();
    assert_eq!(resp_mv1.status(), StatusCode::OK);

    let req_mv2 = Request::builder()
        .method(Method::POST)
        .uri(format!("/player/{}/move/secondteam", p2_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_mv2 = app.clone().oneshot(req_mv2).await.unwrap();
    assert_eq!(resp_mv2.status(), StatusCode::OK);

    // 9. POST /trade3/proposal
    let price1 = fo3_server::state::card_price(p1_spid, 1);
    let price2 = fo3_server::state::card_price(p2_spid, 1);
    let req_prop3 = Request::builder()
        .method(Method::POST)
        .uri("/trade3/proposal")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({
            "id": "default_B01",
            "slotnum": 0,
            "playerids": [
                { "id": p1_id, "price": price1, "spid": p1_spid, "grade": 1 },
                { "id": p2_id, "price": price2, "spid": p2_spid, "grade": 1 }
            ]
        }).to_string()))
        .unwrap();
    let resp_prop3 = app.clone().oneshot(req_prop3).await.unwrap();
    assert_eq!(resp_prop3.status(), StatusCode::OK);
    let bytes_prop3 = to_bytes(resp_prop3.into_body(), usize::MAX).await.unwrap();
    let val_prop3: Value = serde_json::from_slice(&bytes_prop3).unwrap();
    assert_eq!(val_prop3["result"], true);
    assert!(val_prop3.get("props").is_some());
    let props = val_prop3["props"].as_array().unwrap();
    assert!(!props.is_empty());
    assert!(props[0].get("spid").is_some());

    // 10. POST /trade3/contract
    let req_contract3 = Request::builder()
        .method(Method::POST)
        .uri("/trade3/contract")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({ "select": 0 }).to_string()))
        .unwrap();
    let resp_contract3 = app.clone().oneshot(req_contract3).await.unwrap();
    assert_eq!(resp_contract3.status(), StatusCode::OK);
    let bytes_contract3 = to_bytes(resp_contract3.into_body(), usize::MAX).await.unwrap();
    let val_contract3: Value = serde_json::from_slice(&bytes_contract3).unwrap();
    assert_eq!(val_contract3["result"], true);
    assert!(val_contract3.get("resources").is_some());

    // Verify state: p1 and p2 removed, trade state cleared
    {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let st = fo3_server::state::load_state();
        let slot = fo3_server::state::default_slot(&st);
        let team = st["teams"][slot.to_string()].clone();
        let players = team["players"].as_array().unwrap();
        assert!(!players.iter().any(|p| fo3_server::state::iid(p) == p1_id));
        assert!(!players.iter().any(|p| fo3_server::state::iid(p) == p2_id));
        assert!(st.get("trade").is_none());
    }

    // 11. Test Trade 2 Flow: /players/trade2/proposal & /players/trade2/contract
    let (t2_p1, t2_p2) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let slot = fo3_server::state::default_slot(&st);
        let team = st.get_mut("teams").unwrap().get_mut(&slot.to_string()).unwrap();
        let p1 = fo3_server::state::add_player(team, 101000003, 1);
        let p2 = fo3_server::state::add_player(team, 101000004, 1);
        let _ = fo3_server::state::save_state(&st);
        (
            p1["_id"].as_str().unwrap().to_string(),
            p2["_id"].as_str().unwrap().to_string(),
        )
    };

    let req_prop2 = Request::builder()
        .method(Method::POST)
        .uri("/players/trade2/proposal")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({ "players": [t2_p1, t2_p2] }).to_string()))
        .unwrap();
    let resp_prop2 = app.clone().oneshot(req_prop2).await.unwrap();
    assert_eq!(resp_prop2.status(), StatusCode::OK);
    let bytes_prop2 = to_bytes(resp_prop2.into_body(), usize::MAX).await.unwrap();
    let val_prop2: Value = serde_json::from_slice(&bytes_prop2).unwrap();
    assert_eq!(val_prop2["result"], true);
    assert!(val_prop2.get("props").is_some());

    let req_contract2 = Request::builder()
        .method(Method::POST)
        .uri("/players/trade2/contract")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({ "select": 0 }).to_string()))
        .unwrap();
    let resp_contract2 = app.clone().oneshot(req_contract2).await.unwrap();
    assert_eq!(resp_contract2.status(), StatusCode::OK);
    let bytes_contract2 = to_bytes(resp_contract2.into_body(), usize::MAX).await.unwrap();
    let val_contract2: Value = serde_json::from_slice(&bytes_contract2).unwrap();
    assert_eq!(val_contract2["result"], true);

    // 12. Error cases
    // A: Invalid player count (< 2)
    let req_err_count = Request::builder()
        .method(Method::POST)
        .uri("/players/trade2/proposal")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({ "players": ["only_one_id"] }).to_string()))
        .unwrap();
    let resp_err_count = app.clone().oneshot(req_err_count).await.unwrap();
    assert_eq!(resp_err_count.status(), StatusCode::OK);
    let bytes_err = to_bytes(resp_err_count.into_body(), usize::MAX).await.unwrap();
    let val_err: Value = serde_json::from_slice(&bytes_err).unwrap();
    assert_eq!(val_err["result"], false);

    // B: Contract without proposal
    let req_err_contract = Request::builder()
        .method(Method::POST)
        .uri("/trade3/contract")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({ "select": 0 }).to_string()))
        .unwrap();
    let resp_err_contract = app.clone().oneshot(req_err_contract).await.unwrap();
    assert_eq!(resp_err_contract.status(), StatusCode::OK);
    let bytes_err_c = to_bytes(resp_err_contract.into_body(), usize::MAX).await.unwrap();
    let val_err_c: Value = serde_json::from_slice(&bytes_err_c).unwrap();
    assert_eq!(val_err_c["result"], false);
}

#[tokio::test]
async fn test_transfer_price_and_player_pricing() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /transfer/price/:spid/:grade with Alexis Sanchez (spid 88184941, grade 1)
    let spid = 88184941u64;
    let p1 = fo3_server::state::card_price(spid, 1);
    assert!(p1 > 0, "card_price for spid 88184941 grade 1 should be > 0");

    let req_price1 = Request::builder()
        .uri(format!("/transfer/price/{}/1", spid))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_price1 = app.clone().oneshot(req_price1).await.unwrap();
    assert_eq!(resp_price1.status(), StatusCode::OK);
    let bytes_price1 = to_bytes(resp_price1.into_body(), usize::MAX).await.unwrap();
    let val_price1: Value = serde_json::from_slice(&bytes_price1).unwrap();

    assert_eq!(val_price1["upper"], p1);
    assert_eq!(val_price1["default"], p1);
    assert_eq!(val_price1["lowest"], p1);
    assert_eq!(val_price1["lower"], p1 * 8 / 10);
    assert_eq!(val_price1["registrationFee"], 0);

    // 2. GET /transfer/price/:spid/:grade with grade 2
    let p2 = fo3_server::state::card_price(spid, 2);
    assert!(p2 > p1);

    let req_price2 = Request::builder()
        .uri(format!("/transfer/price/{}/2", spid))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_price2 = app.clone().oneshot(req_price2).await.unwrap();
    assert_eq!(resp_price2.status(), StatusCode::OK);
    let bytes_price2 = to_bytes(resp_price2.into_body(), usize::MAX).await.unwrap();
    let val_price2: Value = serde_json::from_slice(&bytes_price2).unwrap();

    assert_eq!(val_price2["upper"], p2);
    assert_eq!(val_price2["default"], p2);
    assert_eq!(val_price2["lowest"], p2);
    assert_eq!(val_price2["lower"], p2 * 8 / 10);
    assert_eq!(val_price2["registrationFee"], 0);

    // 3. GET /transfer/price/:spid/0 (grade 0 yields 0 EP)
    let p0 = fo3_server::state::card_price(spid, 0);
    assert_eq!(p0, 0);

    let req_price0 = Request::builder()
        .uri(format!("/transfer/price/{}/0", spid))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_price0 = app.clone().oneshot(req_price0).await.unwrap();
    assert_eq!(resp_price0.status(), StatusCode::OK);
    let bytes_price0 = to_bytes(resp_price0.into_body(), usize::MAX).await.unwrap();
    let val_price0: Value = serde_json::from_slice(&bytes_price0).unwrap();

    assert_eq!(val_price0["upper"], 0);
    assert_eq!(val_price0["default"], 0);
    assert_eq!(val_price0["lowest"], 0);
    assert_eq!(val_price0["lower"], 0);
    assert_eq!(val_price0["registrationFee"], 0);

    // 4. GET /coach/me - Verify every player in team.players has valid price field
    let req_coach_me = Request::builder()
        .uri("/coach/me")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_coach_me = app.clone().oneshot(req_coach_me).await.unwrap();
    assert_eq!(resp_coach_me.status(), StatusCode::OK);
    let bytes_coach_me = to_bytes(resp_coach_me.into_body(), usize::MAX).await.unwrap();
    let val_coach_me: Value = serde_json::from_slice(&bytes_coach_me).unwrap();

    let players = val_coach_me["team"]["players"].as_array().expect("players array in team");
    assert!(!players.is_empty(), "team.players must not be empty");

    for p in players {
        assert!(p.get("price").is_some(), "player must have price field: {:?}", p);
        let player_spid = p["spid"].as_u64().unwrap();
        let player_grade = p.get("grade").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let expected_price = fo3_server::state::card_price(player_spid, player_grade);
        assert_eq!(p["price"].as_u64().unwrap(), expected_price);
    }

    // 5. GET /coaches - Verify teamvalue is calculated
    let req_coaches = Request::builder()
        .uri("/coaches")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_coaches = app.clone().oneshot(req_coaches).await.unwrap();
    assert_eq!(resp_coaches.status(), StatusCode::OK);
    let bytes_coaches = to_bytes(resp_coaches.into_body(), usize::MAX).await.unwrap();
    let val_coaches: Value = serde_json::from_slice(&bytes_coaches).unwrap();
    let coachlist = val_coaches["coachlist"].as_array().unwrap();
    assert!(!coachlist.is_empty());
    assert!(coachlist[0]["teamvalue"]["val"].as_u64().is_some());

    // 6. GET /transfer/top/50 - Market cards have reasonable prices based on OVR
    let req_top50 = Request::builder()
        .uri("/transfer/top/50")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_top50 = app.clone().oneshot(req_top50).await.unwrap();
    assert_eq!(resp_top50.status(), StatusCode::OK);
    let bytes_top50 = to_bytes(resp_top50.into_body(), usize::MAX).await.unwrap();
    let val_top50: Value = serde_json::from_slice(&bytes_top50).unwrap();
    let top_offers = val_top50.as_array().unwrap();
    assert_eq!(top_offers.len(), 50);
    for offer in top_offers {
        assert!(offer["price"].as_u64().unwrap() >= 1000);
        assert_eq!(offer["grade"], 1);
        assert_eq!(offer["count"], 1);
    }

    // 7. GET /transfer/search/:spid - Search offers for cards
    let req_search = Request::builder()
        .uri(format!("/transfer/search/{}", spid))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_search = app.clone().oneshot(req_search).await.unwrap();
    assert_eq!(resp_search.status(), StatusCode::OK);
    let bytes_search = to_bytes(resp_search.into_body(), usize::MAX).await.unwrap();
    let val_search: Value = serde_json::from_slice(&bytes_search).unwrap();
    let search_offers = val_search.as_array().unwrap();
    assert_eq!(search_offers.len(), 3);
    assert_eq!(search_offers[0]["grade"], 1);
    assert_eq!(search_offers[0]["price"], p1);
    assert_eq!(search_offers[1]["grade"], 2);
    assert_eq!(search_offers[1]["price"], p2);

    // 8. GET /transfer/my/offer/list & /transfer/my/boostfactor
    let req_my_offers = Request::builder()
        .uri("/transfer/my/offer/list")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_my_offers = app.clone().oneshot(req_my_offers).await.unwrap();
    assert_eq!(resp_my_offers.status(), StatusCode::OK);
    let bytes_my_offers = to_bytes(resp_my_offers.into_body(), usize::MAX).await.unwrap();
    let val_my_offers: Value = serde_json::from_slice(&bytes_my_offers).unwrap();
    assert!(val_my_offers.get("offers").is_some());
    assert_eq!(val_my_offers["offerexpireoffset"], 86400000);

    let req_boost = Request::builder()
        .uri("/transfer/my/boostfactor")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_boost = app.clone().oneshot(req_boost).await.unwrap();
    assert_eq!(resp_boost.status(), StatusCode::OK);
    let bytes_boost = to_bytes(resp_boost.into_body(), usize::MAX).await.unwrap();
    let val_boost: Value = serde_json::from_slice(&bytes_boost).unwrap();
    assert_eq!(val_boost["vipDetail"], 0);
}

#[tokio::test]
async fn test_player_enhancement_and_pe2staticvalues() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /static/db/PE2STATICVALUES
    let req_pe2 = Request::builder()
        .uri("/static/db/PE2STATICVALUES")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_pe2 = app.clone().oneshot(req_pe2).await.unwrap();
    assert_eq!(resp_pe2.status(), StatusCode::OK);
    let bytes_pe2 = to_bytes(resp_pe2.into_body(), usize::MAX).await.unwrap();
    let val_pe2: Value = serde_json::from_slice(&bytes_pe2).unwrap();
    assert_eq!(val_pe2["bonus"]["use"], 1000);
    assert_eq!(val_pe2["simplify_step"]["use"], 0);
    assert_eq!(val_pe2["effect_price"]["0"], 10_000_000_000u64);
    assert_eq!(val_pe2["effect_price"]["1"], 1_000_000_000u64);
    assert_eq!(val_pe2["effect_price"]["2"], 100_000_000u64);
    assert_eq!(val_pe2["effect_price"]["3"], 10_000_000u64);
    assert_eq!(val_pe2["effect_price"]["4"], 1_000_000u64);

    // Setup: add two identical cards to test player enhancement
    let p_spid = 100001u64;
    let (p1_id, p2_id) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team_mut(&mut st);
        let p1 = fo3_server::state::add_player(team, p_spid, 1);
        let p2 = fo3_server::state::add_player(team, p_spid, 1);
        fo3_server::state::add_ep(&mut st, 10_000_000);
        fo3_server::state::add_es(&mut st, 1000);
        let _ = fo3_server::state::save_state(&st);
        (
            fo3_server::state::iid(&p1),
            fo3_server::state::iid(&p2),
        )
    };

    // 2. GET /player/:target/enhance/:material/preview
    let req_prev = Request::builder()
        .uri(format!("/player/{}/enhance/{}/preview", p1_id, p2_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_prev = app.clone().oneshot(req_prev).await.unwrap();
    assert_eq!(resp_prev.status(), StatusCode::OK);
    let bytes_prev = to_bytes(resp_prev.into_body(), usize::MAX).await.unwrap();
    let val_prev: Value = serde_json::from_slice(&bytes_prev).unwrap();
    assert_eq!(val_prev["result"], true);
    assert_eq!(val_prev["grade"], 2);
    assert_eq!(val_prev["probability"], 100);
    assert!(val_prev.get("enhancepay").is_some());
    assert!(val_prev.get("price").is_some());

    // 3. GET /player/:target/enhance2.5/:material/preview/:use_point/:item_id/:event_id
    let req_prev25 = Request::builder()
        .uri(format!(
            "/player/{}/enhance2.5/{}/preview/1/-1/1001",
            p1_id, p2_id
        ))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_prev25 = app.clone().oneshot(req_prev25).await.unwrap();
    assert_eq!(resp_prev25.status(), StatusCode::OK);
    let bytes_prev25 = to_bytes(resp_prev25.into_body(), usize::MAX).await.unwrap();
    let val_prev25: Value = serde_json::from_slice(&bytes_prev25).unwrap();
    assert_eq!(val_prev25["result"], true);
    assert_eq!(val_prev25["grade"], 2);
    assert_eq!(val_prev25["probability"], 100);
    assert_eq!(val_prev25["needes"], 20); // 10 * (1 + 1)

    // 4. PUT /player/:target/enhance/:material
    let req_enh = Request::builder()
        .method(Method::PUT)
        .uri(format!("/player/{}/enhance/{}", p1_id, p2_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_enh = app.clone().oneshot(req_enh).await.unwrap();
    assert_eq!(resp_enh.status(), StatusCode::OK);
    let bytes_enh = to_bytes(resp_enh.into_body(), usize::MAX).await.unwrap();
    let val_enh: Value = serde_json::from_slice(&bytes_enh).unwrap();
    assert_eq!(val_enh["result"], true);
    assert_eq!(val_enh["success"], true); // 1+1 rate is 100%
    assert_eq!(val_enh["grade"], 2);
    assert_eq!(val_enh["rgrade"], 2);
    assert!(val_enh.get("resources").is_some());
    let res_enh = val_enh["resources"].as_array().unwrap();
    assert_eq!(res_enh[0]["uri"], "/coach/me");

    // Verify p2 (material) was removed and p1 grade became 2
    {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team(&st);
        let players = team.get("players").and_then(|v| v.as_array()).unwrap();
        assert!(!players.iter().any(|p| fo3_server::state::iid(p) == p2_id));
        let p1 = players.iter().find(|p| fo3_server::state::iid(p) == p1_id).unwrap();
        assert_eq!(fo3_server::state::grade_of(p1), 2);
    }

    // 5. Test enhance 2.5: PUT /player/:target/enhance2.5/:material/:use_point/:item_id/:event_id
    // Add another pair for enhance2.5
    let (p3_id, p4_id) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team_mut(&mut st);
        let p3 = fo3_server::state::add_player(team, p_spid, 1);
        let p4 = fo3_server::state::add_player(team, p_spid, 1);
        let _ = fo3_server::state::save_state(&st);
        (
            fo3_server::state::iid(&p3),
            fo3_server::state::iid(&p4),
        )
    };

    let req_enh25 = Request::builder()
        .method(Method::PUT)
        .uri(format!("/player/{}/enhance2.5/{}/1/-1/1001", p3_id, p4_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_enh25 = app.clone().oneshot(req_enh25).await.unwrap();
    assert_eq!(resp_enh25.status(), StatusCode::OK);
    let bytes_enh25 = to_bytes(resp_enh25.into_body(), usize::MAX).await.unwrap();
    let val_enh25: Value = serde_json::from_slice(&bytes_enh25).unwrap();
    assert_eq!(val_enh25["result"], true);
    assert_eq!(val_enh25["success"], true);
    assert_eq!(val_enh25["grade"], 2);
    assert!(val_enh25.get("resources").is_some());
    let res_enh25 = val_enh25["resources"].as_array().unwrap();
    assert_eq!(res_enh25[0]["uri"], "/coach/me");

    // 6. Test PUT /player/enhance2.5/restore
    // Simulate a failure restore record in state
    {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team_mut(&mut st);
        if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
            if let Some(p) = players.iter_mut().find(|p| fo3_server::state::iid(p) == p3_id) {
                p["grade"] = json!(1); // dropped to 1
            }
        }
        st["enhance_restore"] = json!({
            "id": p3_id,
            "grade": 2,
            "price": 60
        });
        fo3_server::state::add_es(&mut st, 200);
        let _ = fo3_server::state::save_state(&st);
    }

    let req_restore = Request::builder()
        .method(Method::PUT)
        .uri("/player/enhance2.5/restore")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_restore = app.clone().oneshot(req_restore).await.unwrap();
    assert_eq!(resp_restore.status(), StatusCode::OK);
    let bytes_rst = to_bytes(resp_restore.into_body(), usize::MAX).await.unwrap();
    let val_rst: Value = serde_json::from_slice(&bytes_rst).unwrap();
    assert_eq!(val_rst["result"], true);
    assert_eq!(val_rst["grade"], 2);
    assert_eq!(val_rst["rgrade"], 2);
    assert!(val_rst.get("resources").is_some());
    let res_rst = val_rst["resources"].as_array().unwrap();
    assert_eq!(res_rst[0]["uri"], "/coach/me");

    // Verify card grade was restored to 2 and enhance_restore was cleaned up
    {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team(&st);
        let players = team.get("players").and_then(|v| v.as_array()).unwrap();
        let p3 = players.iter().find(|p| fo3_server::state::iid(p) == p3_id).unwrap();
        assert_eq!(fo3_server::state::grade_of(p3), 2);
        assert!(st.get("enhance_restore").is_none());
    }
}

#[tokio::test]
async fn test_inbox_contact_and_bag_endpoints() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /contact/list returns friends, requesting, requested, groups
    let req_contact = Request::builder()
        .uri("/contact/list")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_contact = app.clone().oneshot(req_contact).await.unwrap();
    assert_eq!(resp_contact.status(), StatusCode::OK);
    let bytes_contact = to_bytes(resp_contact.into_body(), usize::MAX).await.unwrap();
    let val_contact: Value = serde_json::from_slice(&bytes_contact).unwrap();
    assert!(val_contact.is_object());
    assert!(val_contact["friends"].is_array());
    assert!(val_contact["requesting"].is_array());
    assert!(val_contact["requested"].is_array());
    assert!(val_contact["groups"].is_array());

    // 2. GET /inbox/my/messages returns empty array initially
    // Clear any messages first
    {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        fo3_server::state::inbox_delete(&mut st, None);
        let _ = fo3_server::state::save_state(&st);
    }
    let req_inbox_empty = Request::builder()
        .uri("/inbox/my/messages")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_inbox_empty = app.clone().oneshot(req_inbox_empty).await.unwrap();
    assert_eq!(resp_inbox_empty.status(), StatusCode::OK);
    let bytes_ie = to_bytes(resp_inbox_empty.into_body(), usize::MAX).await.unwrap();
    let val_ie: Value = serde_json::from_slice(&bytes_ie).unwrap();
    assert_eq!(val_ie["result"], true);
    assert_eq!(val_ie["messages"], json!([]));

    // 3. Add system messages to inbox and verify GET /inbox/my/messages returns them
    let (msg1_id, msg2_id) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let m1 = fo3_server::state::inbox_add(&mut st, "Chào mừng", "Chào mừng bạn đến với máy chủ FO3!", "Hệ thống");
        let m2 = fo3_server::state::inbox_add(&mut st, "Bảo trì", "Bảo trì định kỳ kết thúc thành công.", "Hệ thống");
        let _ = fo3_server::state::save_state(&st);
        (m1["_id"].as_str().unwrap().to_string(), m2["_id"].as_str().unwrap().to_string())
    };

    let req_inbox_msgs = Request::builder()
        .uri("/inbox/my/messages")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_inbox_msgs = app.clone().oneshot(req_inbox_msgs).await.unwrap();
    assert_eq!(resp_inbox_msgs.status(), StatusCode::OK);
    let bytes_im = to_bytes(resp_inbox_msgs.into_body(), usize::MAX).await.unwrap();
    let val_im: Value = serde_json::from_slice(&bytes_im).unwrap();
    assert_eq!(val_im["result"], true);
    let msgs_arr = val_im["messages"].as_array().unwrap();
    assert_eq!(msgs_arr.len(), 2);
    let found_m1 = msgs_arr.iter().find(|m| m["_id"] == msg1_id).unwrap();
    assert_eq!(found_m1["title"], "Chào mừng");
    assert_eq!(found_m1["sender"], "Hệ thống");
    assert_eq!(found_m1["isread"], false);
    assert!(found_m1.get("date").is_some());
    assert!(found_m1.get("senddate").is_some());

    // 4. Mark single message as read via PUT /inbox/my/messages/<id>
    let req_read = Request::builder()
        .method(Method::PUT)
        .uri(format!("/inbox/my/messages/{}", msg1_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_read = app.clone().oneshot(req_read).await.unwrap();
    assert_eq!(resp_read.status(), StatusCode::OK);
    let bytes_read = to_bytes(resp_read.into_body(), usize::MAX).await.unwrap();
    let val_read: Value = serde_json::from_slice(&bytes_read).unwrap();
    assert_eq!(val_read["result"], true);

    // Verify it is marked as read
    let req_get_single = Request::builder()
        .uri(format!("/inbox/my/messages/{}", msg1_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_get_single = app.clone().oneshot(req_get_single).await.unwrap();
    assert_eq!(resp_get_single.status(), StatusCode::OK);
    let bytes_single = to_bytes(resp_get_single.into_body(), usize::MAX).await.unwrap();
    let val_single: Value = serde_json::from_slice(&bytes_single).unwrap();
    assert_eq!(val_single["result"], true);
    assert_eq!(val_single["message"]["isread"], true);

    // 5. Delete single message via DELETE /inbox/my/messages/<id>
    let req_del_single = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/inbox/my/messages/{}", msg1_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_del_single = app.clone().oneshot(req_del_single).await.unwrap();
    assert_eq!(resp_del_single.status(), StatusCode::OK);
    let bytes_ds = to_bytes(resp_del_single.into_body(), usize::MAX).await.unwrap();
    let val_ds: Value = serde_json::from_slice(&bytes_ds).unwrap();
    assert_eq!(val_ds["result"], true);

    // Verify only 1 message remains
    let req_inbox_after_del = Request::builder()
        .uri("/inbox/my/messages")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_inbox_ad = app.clone().oneshot(req_inbox_after_del).await.unwrap();
    let bytes_ad = to_bytes(resp_inbox_ad.into_body(), usize::MAX).await.unwrap();
    let val_ad: Value = serde_json::from_slice(&bytes_ad).unwrap();
    let msgs_ad = val_ad["messages"].as_array().unwrap();
    assert_eq!(msgs_ad.len(), 1);
    assert_eq!(msgs_ad[0]["_id"], msg2_id);

    // 6. Delete all messages via POST with {"_method": "DELETE"}
    let req_del_all = Request::builder()
        .method(Method::POST)
        .uri("/inbox/my/messages")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(r#"{"_method": "DELETE"}"#))
        .unwrap();
    let resp_del_all = app.clone().oneshot(req_del_all).await.unwrap();
    assert_eq!(resp_del_all.status(), StatusCode::OK);
    let bytes_da = to_bytes(resp_del_all.into_body(), usize::MAX).await.unwrap();
    let val_da: Value = serde_json::from_slice(&bytes_da).unwrap();
    assert_eq!(val_da["result"], true);

    // Verify inbox is empty again
    let req_inbox_empty_again = Request::builder()
        .uri("/inbox/my/messages")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_iea = app.clone().oneshot(req_inbox_empty_again).await.unwrap();
    let bytes_iea = to_bytes(resp_iea.into_body(), usize::MAX).await.unwrap();
    let val_iea: Value = serde_json::from_slice(&bytes_iea).unwrap();
    assert_eq!(val_iea["messages"], json!([]));

    // 7. Test /bag, /bag/<id>/unpack, and /bag/unpackall
    // Clear bag first and add 2 distinct gifts
    let (gift1_id, gift2_id, initial_ep) = {
        let _lock = fo3_server::state::STATE_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let mut st = fo3_server::state::load_state();
        let coach = fo3_server::state::my_coach_mut(&mut st);
        coach["bag"] = json!([]);
        let cur_ep = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(0);

        let g1 = fo3_server::state::bag_add(
            &mut st,
            "Quà EP và Thẻ",
            vec![
                json!({"itemid": 1, "count": 100000}),
                json!({"itemid": 28001, "count": 1})
            ],
            vec![],
            "Quà thử nghiệm 1",
        );
        let g2 = fo3_server::state::bag_add(
            &mut st,
            "Quà Thẻ Cầu Thủ",
            vec![json!({"itemid": 1, "count": 50000})],
            vec![],
            "Quà thử nghiệm 2",
        );
        let _ = fo3_server::state::save_state(&st);
        (g1["_id"].as_str().unwrap().to_string(), g2["_id"].as_str().unwrap().to_string(), cur_ep)
    };

    // Verify /bag returns Array with 2 entries
    let req_bag = Request::builder()
        .uri("/bag")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_bag = app.clone().oneshot(req_bag).await.unwrap();
    assert_eq!(resp_bag.status(), StatusCode::OK);
    let bytes_bag = to_bytes(resp_bag.into_body(), usize::MAX).await.unwrap();
    let val_bag: Value = serde_json::from_slice(&bytes_bag).unwrap();
    assert!(val_bag.is_array());
    let bag_arr = val_bag.as_array().unwrap();
    assert_eq!(bag_arr.len(), 2);
    let b1 = bag_arr.iter().find(|e| e["_id"] == gift1_id).unwrap();
    assert_eq!(b1["title"], "Quà EP và Thẻ");
    assert_eq!(b1["type"], "event");
    assert!(b1.get("date").is_some());
    assert!(b1.get("bundleicon").is_some());

    // 8. Unpack gift 1 via POST /bag/<id>/unpack
    let req_unpack_single = Request::builder()
        .method(Method::POST)
        .uri(format!("/bag/{}/unpack", gift1_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_unpack_single = app.clone().oneshot(req_unpack_single).await.unwrap();
    assert_eq!(resp_unpack_single.status(), StatusCode::OK);
    let bytes_us = to_bytes(resp_unpack_single.into_body(), usize::MAX).await.unwrap();
    let val_us: Value = serde_json::from_slice(&bytes_us).unwrap();
    assert_eq!(val_us["result"], true);
    assert_eq!(val_us["overdueitems"], json!([]));
    assert!(val_us.get("resources").is_some());
    let res_us = val_us["resources"].as_array().unwrap();
    assert_eq!(res_us.len(), 2);
    assert_eq!(res_us[0]["uri"], "/coach/me");
    assert_eq!(res_us[1]["uri"], "/bag");

    // Verify coach/me received 100,000 EP
    let coach_me_body = &res_us[0]["body"];
    let new_ep = coach_me_body["money"]["ep"].as_i64().unwrap();
    assert_eq!(new_ep, initial_ep + 100000);

    // Verify /bag in resources only has gift2 remaining
    let rem_bag = res_us[1]["body"].as_array().unwrap();
    assert_eq!(rem_bag.len(), 1);
    assert_eq!(rem_bag[0]["_id"], gift2_id);

    // 9. Unpack remaining via POST /bag/unpackall
    let req_unpackall = Request::builder()
        .method(Method::POST)
        .uri("/bag/unpackall")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(format!(r#"{{"reqUnpackItems": ["{}"]}}"#, gift2_id)))
        .unwrap();
    let resp_unpackall = app.clone().oneshot(req_unpackall).await.unwrap();
    assert_eq!(resp_unpackall.status(), StatusCode::OK);
    let bytes_ua = to_bytes(resp_unpackall.into_body(), usize::MAX).await.unwrap();
    let val_ua: Value = serde_json::from_slice(&bytes_ua).unwrap();
    assert_eq!(val_ua["result"], true);
    assert!(val_ua.get("resources").is_some());
    let res_ua = val_ua["resources"].as_array().unwrap();
    assert_eq!(res_ua[0]["uri"], "/coach/me");
    assert_eq!(res_ua[1]["uri"], "/bag");

    // Verify coach/me received an additional 50,000 EP (total initial + 150,000)
    let coach_me_final = &res_ua[0]["body"];
    let final_ep = coach_me_final["money"]["ep"].as_i64().unwrap();
    assert_eq!(final_ep, initial_ep + 150000);

    // Verify bag is now empty
    let empty_bag = res_ua[1]["body"].as_array().unwrap();
    assert!(empty_bag.is_empty());
}

#[tokio::test]
async fn test_player_leveling_and_expplayer_endpoints() {
    let _test_guard = SERIAL_TEST.lock().await;
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. Test GET /static/db/EXPPLAYER
    let req_ep = Request::builder()
        .method(Method::GET)
        .uri("/static/db/EXPPLAYER")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_ep = app.clone().oneshot(req_ep).await.unwrap();
    assert_eq!(resp_ep.status(), StatusCode::OK);
    let bytes_ep = to_bytes(resp_ep.into_body(), usize::MAX).await.unwrap();
    let val_ep: Value = serde_json::from_slice(&bytes_ep).unwrap();
    assert!(val_ep.is_object());
    let map_ep = val_ep.as_object().unwrap();
    assert_eq!(map_ep.len(), 24, "EXPPLAYER must contain exactly 24 entries (levels 1..=24)");
    for lv in 1..=24 {
        let key = lv.to_string();
        assert!(map_ep.contains_key(&key), "EXPPLAYER missing level {}", key);
        let row = &map_ep[&key];
        assert_eq!(
            row["requiredplayerexp"],
            (100 * lv) as u64,
            "Level {} requiredplayerexp mismatch",
            lv
        );
    }

    // 2. Test GET /static/db/EXPCOACH
    let req_ec = Request::builder()
        .method(Method::GET)
        .uri("/static/db/EXPCOACH")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_ec = app.clone().oneshot(req_ec).await.unwrap();
    assert_eq!(resp_ec.status(), StatusCode::OK);
    let bytes_ec = to_bytes(resp_ec.into_body(), usize::MAX).await.unwrap();
    let val_ec: Value = serde_json::from_slice(&bytes_ec).unwrap();
    assert!(val_ec.is_object());
    let map_ec = val_ec.as_object().unwrap();
    assert_eq!(map_ec.len(), 100, "EXPCOACH must contain exactly 100 entries (levels 1..=100)");
    assert_eq!(map_ec["1"]["totalexp"], 500);
    assert_eq!(map_ec["2"]["totalexp"], 1100);
    assert_eq!(map_ec["3"]["totalexp"], 1800);
    assert_eq!(map_ec["100"]["totalexp"], 545000);

    // 3. Test /coach/me player levelinfo structure: level, exp, to: 100 * level
    let req_me = Request::builder()
        .method(Method::GET)
        .uri("/coach/me")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_me = app.clone().oneshot(req_me).await.unwrap();
    assert_eq!(resp_me.status(), StatusCode::OK);
    let bytes_me = to_bytes(resp_me.into_body(), usize::MAX).await.unwrap();
    let val_me: Value = serde_json::from_slice(&bytes_me).unwrap();
    assert_eq!(val_me["result"], true);

    let players = val_me["team"]["players"]
        .as_array()
        .expect("team.players must be an array");
    assert!(!players.is_empty(), "team.players should not be empty");

    for p in players {
        let li = p.get("levelinfo").expect("player must have levelinfo");
        let lv = li.get("level").and_then(|v| v.as_u64()).expect("levelinfo.level");
        let exp = li.get("exp").and_then(|v| v.as_u64()).expect("levelinfo.exp");
        let to = li.get("to").and_then(|v| v.as_u64()).expect("levelinfo.to");
        assert!(lv >= 1 && lv <= 23, "Player level should be within 1..=23, got {}", lv);
        assert_eq!(to, 100 * lv, "Player levelinfo.to must equal 100 * level");
        if lv == 23 {
            assert_eq!(exp, 0, "Max level player exp must be 0");
        }
    }

    // Coach levelinfo in /coach/me
    let coach_li = val_me.get("levelinfo").expect("coach levelinfo");
    let c_lv = coach_li.get("level").and_then(|v| v.as_u64()).unwrap();
    let c_to = coach_li.get("to").and_then(|v| v.as_u64()).unwrap();
    assert_eq!(c_to, 500 + 100 * (c_lv - 1), "Coach levelinfo.to formula mismatch");

    // 4. Test admin API: player_exp and player_level
    let admin_app = create_admin_router();
    let p0 = &players[0];
    let p0_id = p0["id"].as_str().unwrap();

    // Reset player to level 1 first to make test idempotent across runs
    let req_reset = Request::builder()
        .method(Method::POST)
        .uri("/api/player_level")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(format!(r#"{{"id": "{}", "level": 1}}"#, p0_id)))
        .unwrap();
    let resp_reset = admin_app.clone().oneshot(req_reset).await.unwrap();
    assert_eq!(resp_reset.status(), StatusCode::OK);

    let req_exp = Request::builder()
        .method(Method::POST)
        .uri("/api/player_exp")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(format!(r#"{{"id": "{}", "exp": 150}}"#, p0_id)))
        .unwrap();

    let resp_exp = admin_app.clone().oneshot(req_exp).await.unwrap();
    assert_eq!(resp_exp.status(), StatusCode::OK);
    let bytes_adm_exp = to_bytes(resp_exp.into_body(), usize::MAX).await.unwrap();
    let val_adm_exp: Value = serde_json::from_slice(&bytes_adm_exp).unwrap();
    assert_eq!(val_adm_exp["ok"], true);
    assert_eq!(val_adm_exp["current"]["level"], 2);
    assert_eq!(val_adm_exp["current"]["exp"], 50);
    assert_eq!(val_adm_exp["current"]["to"], 200);

    let req_set_level = Request::builder()
        .method(Method::POST)
        .uri("/api/player_level")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(format!(r#"{{"id": "{}", "level": 10}}"#, p0_id)))
        .unwrap();

    let resp_sl = admin_app.clone().oneshot(req_set_level).await.unwrap();
    assert_eq!(resp_sl.status(), StatusCode::OK);
    let bytes_sl = to_bytes(resp_sl.into_body(), usize::MAX).await.unwrap();
    let val_sl: Value = serde_json::from_slice(&bytes_sl).unwrap();
    assert_eq!(val_sl["ok"], true);
    assert_eq!(val_sl["levelinfo"]["level"], 10);
    assert_eq!(val_sl["levelinfo"]["to"], 1000);

    // Reset player back to level 1 for state cleanliness
    let req_cleanup = Request::builder()
        .method(Method::POST)
        .uri("/api/player_level")
        .header(header::HOST, "127.0.0.1:8088")
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-FO3-Admin", "1")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(format!(r#"{{"id": "{}", "level": 1}}"#, p0_id)))
        .unwrap();
    let _ = admin_app.clone().oneshot(req_cleanup).await.unwrap();

    // 5. Test Match End and Reward
    let match_id = "507f1f77bcf86cd799439011";
    let match_stats = json!({
        "myTeam": "home",
        "opponentTeam": "away",
        "home": {
            "score": 3,
            "players": [
                {"spid": p0["spid"], "goals": 2, "assists": 1}
            ]
        },
        "away": {
            "score": 1,
            "players": []
        }
    });

    let req_match_end = Request::builder()
        .method(Method::PUT)
        .uri(format!("/match/{}/end", match_id))
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(match_stats.to_string()))
        .unwrap();

    let resp_me = app.clone().oneshot(req_match_end).await.unwrap();
    assert_eq!(resp_me.status(), StatusCode::OK);
    let bytes_m_end = to_bytes(resp_me.into_body(), usize::MAX).await.unwrap();
    let val_m_end: Value = serde_json::from_slice(&bytes_m_end).unwrap();
    assert_eq!(val_m_end["result"], true);
    assert!(val_m_end["match"]["reward"]["players"].is_array());
    assert!(val_m_end["match"]["reward"]["coach"].is_object());

    // GET /match/<id> returns the completed match
    let req_match_get = Request::builder()
        .method(Method::GET)
        .uri(format!("/match/{}", match_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let resp_m_get = app.clone().oneshot(req_match_get).await.unwrap();
    assert_eq!(resp_m_get.status(), StatusCode::OK);
    let bytes_m_get = to_bytes(resp_m_get.into_body(), usize::MAX).await.unwrap();
    let val_m_get: Value = serde_json::from_slice(&bytes_m_get).unwrap();
    assert_eq!(val_m_get["id"], match_id);
    assert_eq!(val_m_get["stage"], "closed");
}

#[tokio::test]
async fn test_market_endpoints_full_lifecycle() {
    let _test_guard = SERIAL_TEST.lock().await;
    {
        let mut st = fo3_server::state::load_state();
        let slot = fo3_server::state::default_slot(&st);
        if let Some(players) = st.get_mut("teams").and_then(|t| t.get_mut(&slot.to_string())).and_then(|t| t.get_mut("players")).and_then(|p| p.as_array_mut()) {
            if players.len() > 50 {
                players.truncate(36);
                let _ = fo3_server::state::save_state(&st);
            }
        }
    }
    let app = create_http_router();
    let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

    // 1. GET /transfer/top/50
    let req_top = Request::builder()
        .uri("/transfer/top/50")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_top = app.clone().oneshot(req_top).await.unwrap();
    assert_eq!(resp_top.status(), StatusCode::OK);
    let bytes_top = to_bytes(resp_top.into_body(), usize::MAX).await.unwrap();
    let val_top: Value = serde_json::from_slice(&bytes_top).unwrap();
    let top_arr = val_top.as_array().expect("transfer/top/50 must return an array");
    assert_eq!(top_arr.len(), 50);
    assert!(top_arr[0].get("spid").is_some());
    assert_eq!(top_arr[0]["expandme"], "player");
    assert_eq!(top_arr[0]["grade"], 1);
    assert_eq!(top_arr[0]["count"], 1);
    assert!(top_arr[0]["price"].as_u64().unwrap() > 0);

    // 2. GET /transfer/top/50/position/FW
    let req_top_fw = Request::builder()
        .uri("/transfer/top/50/position/FW")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_top_fw = app.clone().oneshot(req_top_fw).await.unwrap();
    assert_eq!(resp_top_fw.status(), StatusCode::OK);
    let bytes_top_fw = to_bytes(resp_top_fw.into_body(), usize::MAX).await.unwrap();
    let val_top_fw: Value = serde_json::from_slice(&bytes_top_fw).unwrap();
    let top_fw_arr = val_top_fw.as_array().unwrap();
    assert_eq!(top_fw_arr.len(), 50);

    // 3. GET /transfer/search/88207995 (marketable) and 158023 (unmarketable)
    let req_search = Request::builder()
        .uri("/transfer/search/88207995")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_search = app.clone().oneshot(req_search).await.unwrap();
    assert_eq!(resp_search.status(), StatusCode::OK);
    let bytes_search = to_bytes(resp_search.into_body(), usize::MAX).await.unwrap();
    let val_search: Value = serde_json::from_slice(&bytes_search).unwrap();
    let search_arr = val_search.as_array().unwrap();
    assert_eq!(search_arr.len(), 3);
    assert_eq!(search_arr[0]["grade"], 1);
    assert_eq!(search_arr[1]["grade"], 2);
    assert_eq!(search_arr[2]["grade"], 3);

    let req_search_unm = Request::builder()
        .uri("/transfer/search/158023")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_search_unm = app.clone().oneshot(req_search_unm).await.unwrap();
    assert_eq!(resp_search_unm.status(), StatusCode::OK);
    let bytes_unm = to_bytes(resp_search_unm.into_body(), usize::MAX).await.unwrap();
    let val_unm: Value = serde_json::from_slice(&bytes_unm).unwrap();
    assert!(val_unm.as_array().unwrap().is_empty());

    // 4. GET /transfer/my/boostfactor
    let req_boost = Request::builder()
        .uri("/transfer/my/boostfactor")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_boost = app.clone().oneshot(req_boost).await.unwrap();
    assert_eq!(resp_boost.status(), StatusCode::OK);
    let bytes_boost = to_bytes(resp_boost.into_body(), usize::MAX).await.unwrap();
    let val_boost: Value = serde_json::from_slice(&bytes_boost).unwrap();
    assert_eq!(val_boost["igr"], 0.2);
    assert_eq!(val_boost["vip"], 0.3);
    assert_eq!(val_boost["event"], 0);
    assert_eq!(val_boost["item"], 0);
    assert_eq!(val_boost["discountlimit"], 0);

    // 5. GET /transfer/my/offer/list
    let req_list = Request::builder()
        .uri("/transfer/my/offer/list")
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_list = app.clone().oneshot(req_list).await.unwrap();
    assert_eq!(resp_list.status(), StatusCode::OK);
    let bytes_list = to_bytes(resp_list.into_body(), usize::MAX).await.unwrap();
    let val_list: Value = serde_json::from_slice(&bytes_list).unwrap();
    assert_eq!(val_list["offerexpireoffset"], 86400000);
    assert_eq!(val_list["boost"]["igr"], 0.2);
    assert!(val_list["offers"].is_array());

    // 6. Setup coach EP for purchase
    {
        let mut st = fo3_server::state::load_state();
        fo3_server::state::add_ep(&mut st, 50_000_000);
        let _ = fo3_server::state::save_state(&st);
    }

    // 7. POST /transfer/purchase/:spid/:grade/:price/purchase
    let target_spid = 88207995u64;
    let card_price = fo3_server::state::card_price(target_spid, 1);
    let req_purchase = Request::builder()
        .method(Method::POST)
        .uri(format!("/transfer/purchase/{}/1/{}/purchase", target_spid, card_price))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_purchase = app.clone().oneshot(req_purchase).await.unwrap();
    assert_eq!(resp_purchase.status(), StatusCode::OK);
    let bytes_pur = to_bytes(resp_purchase.into_body(), usize::MAX).await.unwrap();
    let val_pur: Value = serde_json::from_slice(&bytes_pur).unwrap();
    assert_eq!(val_pur["result"], true);
    assert!(val_pur.get("resources").is_some());
    let pur_res = val_pur["resources"].as_array().unwrap();
    assert_eq!(pur_res[0]["uri"], "/coach/me");

    // Find the purchased card ID (it's in reserves)
    let purchased_card_id = {
        let st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team(&st);
        let players = team["players"].as_array().unwrap();
        let p = players.iter().rev().find(|p| p["spid"] == target_spid).unwrap();
        fo3_server::state::iid(p)
    };

    // 8. POST /transfer/offer with starter card -> error
    let starter_id = {
        let st = fo3_server::state::load_state();
        let team = fo3_server::state::my_team(&st);
        let pos = team["squad"]["positions"].as_array().unwrap();
        pos[0]["id"].as_str().unwrap().to_string()
    };
    let req_offer_starter = Request::builder()
        .method(Method::POST)
        .uri("/transfer/offer")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({
            "playerobjectid": starter_id,
            "price": 10000
        }).to_string()))
        .unwrap();
    let resp_off_starter = app.clone().oneshot(req_offer_starter).await.unwrap();
    assert_eq!(resp_off_starter.status(), StatusCode::OK);
    let bytes_off_s = to_bytes(resp_off_starter.into_body(), usize::MAX).await.unwrap();
    let val_off_s: Value = serde_json::from_slice(&bytes_off_s).unwrap();
    assert_eq!(val_off_s["result"], false);
    assert_eq!(val_off_s["reason"], "TRANSFERMARKET_SELECT_SELLABLE_PLAYER");

    // 9. POST /transfer/offer with purchased reserve card
    let req_offer_ok = Request::builder()
        .method(Method::POST)
        .uri("/transfer/offer")
        .header(header::HOST, "APSEODESK172")
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::from(json!({
            "playerobjectid": purchased_card_id,
            "price": card_price
        }).to_string()))
        .unwrap();
    let resp_off_ok = app.clone().oneshot(req_offer_ok).await.unwrap();
    assert_eq!(resp_off_ok.status(), StatusCode::OK);
    let bytes_off_ok = to_bytes(resp_off_ok.into_body(), usize::MAX).await.unwrap();
    let val_off_ok: Value = serde_json::from_slice(&bytes_off_ok).unwrap();
    assert_eq!(val_off_ok["result"], true);
    let off_resources = val_off_ok["resources"].as_array().unwrap();
    assert_eq!(off_resources[0]["uri"], "/transfer/my/offer/list");
    let listed_offers = off_resources[0]["body"]["offers"].as_array().unwrap();
    let active_offer = listed_offers.iter().find(|o| o["spid"] == target_spid).unwrap();
    assert_eq!(active_offer["status"], "sold");
    let offer_id = active_offer["_id"].as_str().unwrap().to_string();

    // 10. DELETE /transfer/my/offer/completed/:offerid
    let req_collect = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/transfer/my/offer/completed/{}", offer_id))
        .header(header::HOST, "APSEODESK172")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();
    let resp_collect = app.clone().oneshot(req_collect).await.unwrap();
    assert_eq!(resp_collect.status(), StatusCode::OK);
    let bytes_col = to_bytes(resp_collect.into_body(), usize::MAX).await.unwrap();
    let val_col: Value = serde_json::from_slice(&bytes_col).unwrap();
    assert_eq!(val_col["result"], true);
    assert!(val_col["ep"].as_u64().unwrap() > 0);
    let col_resources = val_col["resources"].as_array().unwrap();
    assert_eq!(col_resources[0]["uri"], "/transfer/my/offer/list");
    assert_eq!(col_resources[1]["uri"], "/coach/me");

    // Verify offer is no longer in offers list
    let remaining_offers = col_resources[0]["body"]["offers"].as_array().unwrap();
    assert!(!remaining_offers.iter().any(|o| o["_id"] == offer_id));
}
