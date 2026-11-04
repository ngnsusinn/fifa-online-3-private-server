use crate::state;
use axum::{
    body::Bytes,
    extract::ConnectInfo,
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Router,
};
use chrono::Utc;
use flate2::read::GzDecoder;
use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const CDN_PREFIX: &str = "cdn/";

fn get_assets_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FO3_ASSETS") {
        return PathBuf::from(dir);
    }
    let candidates = [
        std::env::var("FO3_GAME").unwrap_or_default(),
        "FO3VN".to_string(),
        r"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN".to_string(),
        r"D:\FO3ID\32795".to_string(),
    ];
    for base in candidates {
        if base.is_empty() {
            continue;
        }
        let p = Path::new(&base);
        for sub in ["fo3sg", "fo3vn", "fo3id"] {
            let candidate = p.join("_cache").join("games").join(sub);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("cdn")
}

fn cdn_file(rel: &str) -> Option<(&'static str, Vec<u8>)> {
    if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
        return None;
    }
    let base = get_assets_dir();
    let file_path = base.join(rel);
    if !file_path.is_file() {
        return None;
    }
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let ctype = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "swf" => "application/x-shockwave-flash",
        "json" => "application/json",
        "xml" => "text/xml",
        _ => "application/octet-stream",
    };
    fs::read(&file_path).ok().map(|data| (ctype, data))
}

fn from_responses_file(path: &str) -> Option<Vec<u8>> {
    let sanitized = path.trim_matches('/').replace('/', "_") + ".json";
    let resp_path = Path::new("responses").join(&sanitized);
    if resp_path.is_file() {
        fs::read(&resp_path).ok()
    } else {
        None
    }
}

fn log_request(method: &Method, path: &str, addr: &SocketAddr, host: &str, body: &[u8]) {
    let now = chrono::Local::now().format("%H:%M:%S").to_string();
    let body_str = String::from_utf8_lossy(body);
    let preview: String = body_str.chars().take(2000).collect();
    let line = format!(
        "{} {} {} from {} host={} len={} body={:?}",
        now,
        method,
        path,
        addr.ip(),
        host,
        body.len(),
        preview
    );
    println!("{}", line);
    if let Some(parent) = Path::new(state::LOG_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state::LOG_PATH)
    {
        let _ = writeln!(f, "{}", line);
    }
}

static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn get_patterns() -> &'static Vec<(Regex, &'static str)> {
    PATTERNS.get_or_init(|| {
        vec![
            (Regex::new(r"^/item/(\w+)/use$").unwrap(), "item_use"),
            (Regex::new(r"^/bag/(\w+)/unpack$").unwrap(), "bag_unpack"),
            (
                Regex::new(r"^/coach/(\w+)/postponed$").unwrap(),
                "coach_postponed",
            ),
            (Regex::new(r"^/stadium/(\d+)$").unwrap(), "stadium"),
            (
                Regex::new(r"^/static/info/authentic/team/(licensed|all)/(\d+)$").unwrap(),
                "authentic_team",
            ),
            (
                Regex::new(r"^/static/info/team/summary/(\d+)$").unwrap(),
                "team_summary",
            ),
            (
                Regex::new(r"^/player/(\w+)/move/(firstteam|secondteam)$").unwrap(),
                "player_move",
            ),
            (
                Regex::new(r"^/player/(\w+)/enhance/(\w+)/preview$").unwrap(),
                "enhance_preview",
            ),
            (
                Regex::new(r"^/league2/sponsor/(\d+)$").unwrap(),
                "league2_sponsor",
            ),
            (
                Regex::new(r"^/cup/([0-9a-f]{24})/match$").unwrap(),
                "cup_match",
            ),
            (
                Regex::new(r"^/cup/([0-9a-f]{24})/next$").unwrap(),
                "cup_next",
            ),
            (
                Regex::new(r"^/cup/([0-9a-f]{24})/(?:resume|test)$").unwrap(),
                "cup_resume",
            ),
            (
                Regex::new(r"^/cup/([0-9a-f]{24})/leave$").unwrap(),
                "cup_leave",
            ),
            (Regex::new(r"^/cup/([0-9a-f]{24})$").unwrap(), "cup_id"),
        ]
    })
}

fn handle_exact_or_pattern(
    method: &Method,
    path: &str,
    _query: &str,
    body: &[u8],
) -> Option<Value> {
    match path {
        "/info/server/config" => Some(json!({
            "redirector": {"host": "127.0.0.1", "port": 28000},
            "notification": {"host": "127.0.0.1", "port": 28001},
            "region": "ID",
            "status": "ok"
        })),
        "/info/server/locale" => {
            let lang = std::env::var("FO3_LANGUAGE").unwrap_or_else(|_| {
                if Path::new("FO3VN").exists()
                    || Path::new(r"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN").exists()
                {
                    "vie".to_string()
                } else {
                    "ind".to_string()
                }
            });
            Some(json!({
                "result": true,
                "language": lang,
                "url": format!("https://APSEODESK172/{}", CDN_PREFIX)
            }))
        }
        "/info/server/time" => Some(json!({
            "timestamp": Utc::now().timestamp_millis(),
            "timezone": -420
        })),
        "/game/my/ipaddress" => Some(json!({
            "result": true,
            "ipaddress": "127.0.0.1"
        })),
        "/publisher/garena/sign" => Some(json!({
            "result": "ok",
            "token": "LOCAL-SIGNED-TOKEN",
            "uid": 1
        })),
        "/publisher/nexon/2ndauth" => Some(json!({
            "result": "ok",
            "passport": "LOCAL-PASSPORT"
        })),
        "/telemetry/gameconnection" => Some(json!({"result": true})),
        "/shop/cash/charge/url" | "/shop/cash/history/url" => Some(json!({"url": "about:blank"})),
        "/coaches" => {
            let st = state::load_state();
            Some(json!({
                "coaches": st.get("coaches").cloned().unwrap_or(json!([])),
                "defaultcoachid": st.get("defaultcoachid").cloned().unwrap_or(json!(""))
            }))
        }
        "/coach/me" | "/coach/me/onready" => {
            let mut st = state::load_state();
            let coach = state::my_coach(&st).clone();
            let team = state::my_team(&st).clone();
            let ep = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(state::START_EP);
            // Grant daily login reward if available
            let _ = state::daily_login(&mut st);
            let _ = state::save_state(&st);
            Some(json!({
                "result": true,
                "coach": coach,
                "team": team,
                "money": {"ep": ep, "point": 0, "cash": 0}
            }))
        }
        "/coach/me/squad" => {
            let mut st = state::load_state();
            if method == Method::PUT || method == Method::POST {
                if let Ok(v) = serde_json::from_slice::<Value>(body) {
                    let team = state::my_team_mut(&mut st);
                    team["squad"] = v;
                    let _ = state::save_state(&st);
                }
            }
            let team = state::my_team(&st);
            Some(team.get("squad").cloned().unwrap_or(json!({})))
        }
        "/coach/0/default" | "/coach/1/default" => {
            let mut st = state::load_state();
            let slot: usize = if path.contains("/1/") { 1 } else { 0 };
            if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
                if let Some(c) = coaches.iter().find(|c| c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64)) {
                    if let Some(id) = c.get("_id").and_then(|v| v.as_str()) {
                        st["defaultcoachid"] = json!(id);
                        let _ = state::save_state(&st);
                    }
                }
            }
            Some(json!({"result": true}))
        }
        "/account" => {
            let st = state::load_state();
            let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
            Some(json!({
                "result": true,
                "account": {"uid": 1, "coachid": def_id}
            }))
        }
        "/account/option" => Some(json!({"result": true})),
        "/events" => Some(json!({
            "events": [{"id": 1, "type": "enhance", "name": "Event"}]
        })),
        "/shop/list" => Some(json!({"products": []})),
        "/bag" => {
            let st = state::load_state();
            let coach = state::my_coach(&st);
            Some(json!({
                "bag": coach.get("bag").cloned().unwrap_or(json!([]))
            }))
        }
        "/bag/unpackall" => Some(json!({"result": true})),
        _ => {
            // Check regex pattern routes
            for (re, name) in get_patterns() {
                if let Some(caps) = re.captures(path) {
                    return match *name {
                        "item_use" => Some(json!({"result": true})),
                        "bag_unpack" => Some(json!({"result": true})),
                        "coach_postponed" => Some(json!({"result": true})),
                        "stadium" => {
                            let id = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                            Some(json!({"result": true, "id": id.parse::<u64>().unwrap_or(1)}))
                        }
                        "authentic_team" => Some(json!({"result": true})),
                        "team_summary" => {
                            let id = caps.get(1).map(|m| m.as_str()).unwrap_or("1");
                            Some(json!({"id": id.parse::<u64>().unwrap_or(1), "result": true}))
                        }
                        "player_move" => Some(json!({"result": true})),
                        "enhance_preview" => Some(json!({"result": true, "rate": 100})),
                        "league2_sponsor" => Some(json!({"result": true})),
                        "cup_match" | "cup_next" | "cup_resume" | "cup_leave" => {
                            Some(json!({"result": true}))
                        }
                        "cup_id" => {
                            let cupid = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                            let st = state::load_state();
                            let cups = st.get("cups").and_then(|c| c.get(cupid)).cloned();
                            Some(cups.unwrap_or(json!({})))
                        }
                        _ => None,
                    };
                }
            }
            None
        }
    }
}

pub async fn http_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    raw_body: Bytes,
) -> Response {
    // 1. Gzip decompression
    let mut body = raw_body.to_vec();
    if body.len() >= 2 && body[0] == 0x1f && body[1] == 0x8b {
        let mut decompressed = Vec::new();
        let mut gz = GzDecoder::new(&body[..]);
        if gz.read_to_end(&mut decompressed).is_ok() {
            body = decompressed;
        }
    }

    let path = uri.path();
    let query = uri.query().unwrap_or("");
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    // 2. Request logging
    log_request(&method, path, &addr, host, &body);

    // 3. /cdn/ static asset serving
    if path.starts_with("/cdn/") {
        let rel = &path["/cdn/".len()..];
        if let Some((ctype, data)) = cdn_file(rel) {
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, ctype)],
                data,
            )
                .into_response();
        } else {
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    // 4. Synchronize state operations
    let _lock = state::STATE_MUTEX.lock().unwrap();

    // 5. Static responses/<path>.json fallback
    if let Some(file_data) = from_responses_file(path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            file_data,
        )
            .into_response();
    }

    // 6. Exact endpoints or pattern routes
    if let Some(val) = handle_exact_or_pattern(&method, path, query, &body) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            val.to_string(),
        )
            .into_response();
    }

    // 7. Unknown route fallback:
    // Unknown POST returns {"result": true}
    // Unknown GET / others return {}
    let fallback = if method == Method::POST {
        json!({"result": true})
    } else {
        json!({})
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        fallback.to_string(),
    )
        .into_response()
}

pub fn create_http_router() -> Router {
    Router::new().fallback(http_handler)
}
