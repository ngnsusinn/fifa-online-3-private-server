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
    // Dedicated clean assets for localization: ensure real localization bigs are served
    // rather than potentially corrupted cache files.
    if rel == "externalAssets/language_eng.big" {
        let p = Path::new("FO3VN").join("data").join("ui").join("localization").join("eng.big");
        if p.is_file() {
            return fs::read(&p).ok().map(|d| ("application/octet-stream", d));
        }
    } else if rel == "externalAssets/language_vie.big" {
        let p = Path::new("FO3VN").join("data").join("ui").join("localization").join("vie.big");
        if p.is_file() {
            return fs::read(&p).ok().map(|d| ("application/octet-stream", d));
        }
    }
    let base = get_assets_dir();
    let file_path = base.join(rel);
    let target = if file_path.is_file() {
        file_path
    } else {
        let alt = Path::new("FO3VN").join("data").join("ui").join(rel);
        if alt.is_file() {
            alt
        } else {
            let alt2 = Path::new("FO3VN").join("data").join("ui").join("externalAssets").join(rel);
            if alt2.is_file() {
                alt2
            } else if rel == "externalAssets/language_eng.big" {
                let p = Path::new("FO3VN").join("data").join("ui").join("localization").join("eng.big");
                if p.is_file() {
                    return fs::read(&p).ok().map(|d| ("application/octet-stream", d));
                }
                return None;
            } else if rel == "externalAssets/language_vie.big" {
                let p = Path::new("FO3VN").join("data").join("ui").join("localization").join("vie.big");
                if p.is_file() {
                    return fs::read(&p).ok().map(|d| ("application/octet-stream", d));
                }
                return None;
            } else {
                return None;
            }
        }
    };
    let ext = target
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
        "usm" => "application/octet-stream",
        _ => "application/octet-stream",
    };
    fs::read(&target).ok().map(|data| (ctype, data))
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
            (Regex::new(r"^/game/inweburl/(\w+)$").unwrap(), "inweburl"),
            (Regex::new(r"^/static/bescript/(\w+)$").unwrap(), "bescript"),
            (Regex::new(r"^/game/my/tactic/(\d+)$").unwrap(), "tactic_slot"),
            (Regex::new(r"^/game/my/team/kickers/(\d+)$").unwrap(), "team_kickers"),
            (Regex::new(r"^/coach/(\d+)/new/step/3$").unwrap(), "coach_new_team"),
            (Regex::new(r"^/coach/(\d+)$").unwrap(), "coach_create"),
        ]
    })
}

fn default_tactic(id: usize) -> Value {
    json!({
        "id": id,
        "name": "",
        "teammentality": 3,
        "bus": {"buildupspeed": 50, "passing": 50, "positioning": 0},
        "cc": {"passing": 50, "crossing": 50, "shooting": 50, "positioning": 0},
        "def": {"mentality": 50, "aggression": 50, "teamwidth": 50, "defenderline": 0}
    })
}

fn build_coach_me(st: &mut Value, slot: usize) -> Value {
    let coach = if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
        coaches
            .iter()
            .find(|c| c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64))
            .cloned()
            .unwrap_or_else(|| state::my_coach(st).clone())
    } else {
        state::my_coach(st).clone()
    };

    let mut team = st
        .get("teams")
        .and_then(|t| t.get(&slot.to_string()))
        .cloned()
        .unwrap_or_else(|| state::my_team(st).clone());

    let ep = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(state::START_EP);
    let teamid = coach.get("teamid").and_then(|v| v.as_u64()).unwrap_or(1);

    if team.get("players").and_then(|p| p.as_array()).map(|a| a.is_empty()).unwrap_or(true) {
        team = state::get_default_team(teamid);
    }

    let cid = coach
        .get("_id")
        .and_then(|v| v.as_str())
        .unwrap_or("000000000000000000000001")
        .to_string();
    let cname = coach
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Manager")
        .to_string();

    let summary = from_responses_file(&format!("/static/info/authentic/team/all/{}", teamid))
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.get("summary").cloned())
        .unwrap_or_else(|| json!({
            "id": teamid,
            "teamid": teamid,
            "name": "Arsenal",
            "league": {"id": 13, "name": "Premier League"},
            "nation": {"id": 14, "name": "England"},
            "ovr": {"DF": 74, "MF": 73, "FW": 75},
            "teamgrade": 4.5
        }));

    let squad_val = team.get("squad").cloned().unwrap_or_else(|| json!({
        "positions": [],
        "kickers": ["", "", "", "", ""],
        "captain": ""
    }));

    if team.get("captain").is_none() || team["captain"].as_str().map(|s| s.is_empty()).unwrap_or(true) {
        if let Some(cap) = squad_val.get("captain") {
            team["captain"] = cap.clone();
        }
    }
    if team.get("kickers").is_none() || team["kickers"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
        if let Some(k) = squad_val.get("kickers") {
            team["kickers"] = k.clone();
        }
    }
    if team.get("presetidx").is_none() {
        team["presetidx"] = json!(0);
    }
    if team.get("formations").is_none() {
        team["formations"] = json!([]);
    }

    json!({
        "result": true,
        "id": cid,
        "_id": cid,
        "name": cname,
        "slot": slot,
        "teamid": teamid,
        "team": team.clone(),
        "coach": coach.clone(),
        "money": {
            "ep": ep,
            "es": 0,
            "cash": 0,
            "mp": 0,
            "useable_mp": 0,
            "mpdata": {"point": 0, "until": "2030-01-01T00:00:00.000Z"}
        },
        "igr": true,
        "vipDetail": 2,
        "vipExpireAt": 2051222400000i64,
        "vipStatus": {"VIP1": 2051222400000i64},
        "sp": {"point": 0},
        "gpdata": {"gp": 0},
        "badges": [],
        "currentbadge": 0,
        "levelinfo": {"level": 1, "exp": 0, "need": 600},
        "statistics": {},
        "league": {"leagueid": 13},
        "club": {},
        "uimode": 0,
        "limits": {"cups": 3, "players": 200, "baseplayers": 200, "firstTeam": 36},
        "transferfavorites": [],
        "currdivision": "",
        "playlimit": {},
        "kit": {
            "availables": {
                "home": {"id": (teamid << 5)},
                "away": {"id": (teamid << 5 | 1)},
                "goalie": {"id": (teamid << 5 | 2)}
            }
        },
        "summary": summary,
        "career": {"appat": "2026-11-04T00:00:00.000Z"},
        "stadium": {"id": 1},
        "items": [],
        "matchstats": [],
        "scouters": [],
        "cups": [],
        "formations": team.get("formations").cloned().unwrap_or(json!([])),
        "teamsquadlist": {"preset": [squad_val]}
    })
}

fn handle_exact_or_pattern(
    method: &Method,
    path: &str,
    _query: &str,
    body: &[u8],
    host: &str,
) -> Option<Value> {
    match path {
        "/info/server/config" => {
            let region = std::env::var("FO3_REGION").unwrap_or_else(|_| {
                let lang = std::env::var("FO3_LANGUAGE").unwrap_or_default();
                if lang.eq_ignore_ascii_case("eng") {
                    return "SG".to_string();
                }
                if Path::new("FO3VN").exists()
                    || Path::new(r"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN").exists()
                {
                    "VN".to_string()
                } else {
                    "ID".to_string()
                }
            });
            Some(json!({
                "redirector": {"host": "127.0.0.1", "port": 28000},
                "notification": {"host": "127.0.0.1", "port": 28001},
                "region": region,
                "status": "ok"
            }))
        },
        "/info/server/locale" => {
            let lang = std::env::var("FO3_LANGUAGE").unwrap_or_else(|_| {
                let reg = std::env::var("FO3_REGION").unwrap_or_default();
                if reg.eq_ignore_ascii_case("SG") {
                    "eng".to_string()
                } else {
                    "vie".to_string()
                }
            });
            let url = if host.contains("8080") {
                format!("http://127.0.0.1:8080/{}", CDN_PREFIX)
            } else if !host.is_empty() {
                format!("https://{}/{}", host.split(':').next().unwrap_or("127.0.0.1"), CDN_PREFIX)
            } else {
                format!("https://127.0.0.1/{}", CDN_PREFIX)
            };
            Some(json!({
                "result": true,
                "language": lang,
                "url": url
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
            let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
            let coaches_arr = st.get("coaches").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            let mut list = Vec::new();
            for c in &coaches_arr {
                let slot = c.get("slot").and_then(|v| v.as_u64()).unwrap_or(0);
                let teamid = c.get("teamid").and_then(|v| v.as_u64()).unwrap_or(1);

                // Load summary for teamid
                let summary = from_responses_file(&format!("/static/info/authentic/team/all/{}", teamid))
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .and_then(|v| v.get("summary").cloned())
                    .unwrap_or_else(|| json!({
                        "id": teamid,
                        "teamid": teamid,
                        "name": "Arsenal",
                        "league": {"id": 13, "name": "Premier League"},
                        "nation": {"id": 14, "name": "England"},
                        "ovr": {"DF": 74, "MF": 73, "FW": 75},
                        "teamgrade": 4.5
                    }));

                let mut entry = c.clone();
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("state".to_string(), json!("active"));
                    obj.insert("summary".to_string(), summary);
                    obj.insert("teamvalue".to_string(), json!({"val": 0}));
                    obj.insert("levelinfo".to_string(), json!({"level": 1, "exp": 0, "need": 600}));
                    obj.insert("recoveryperiod".to_string(), json!(300));
                    obj.insert("slot".to_string(), json!(slot));
                    obj.insert("teamid".to_string(), json!(teamid));
                }
                list.push(entry);
            }
            Some(json!({
                "coachlist": list.clone(),
                "coaches": list,
                "defaultcoachid": def_id
            }))
        }
        "/coach/me" => {
            let mut st = state::load_state();
            let _ = state::daily_login(&mut st);
            let _ = state::save_state(&st);
            let slot = if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
                let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
                coaches.iter()
                    .find(|c| c.get("_id").and_then(|v| v.as_str()) == Some(def_id))
                    .and_then(|c| c.get("slot").and_then(|v| v.as_u64()))
                    .unwrap_or(0) as usize
            } else {
                0
            };
            Some(build_coach_me(&mut st, slot))
        }
        "/coach/me/onready" => {
            let mut st = state::load_state();
            let slot = if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
                let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
                coaches.iter()
                    .find(|c| c.get("_id").and_then(|v| v.as_str()) == Some(def_id))
                    .and_then(|c| c.get("slot").and_then(|v| v.as_u64()))
                    .unwrap_or(0) as usize
            } else {
                0
            };
            let body = build_coach_me(&mut st, slot);
            Some(json!({
                "result": true,
                "resources": [
                    {
                        "uri": "/coach/me",
                        "body": body
                    }
                ]
            }))
        }
        "/coach/me/squad" => {
            let mut st = state::load_state();
            let slot = state::default_slot(&st);
            if method == Method::PUT || method == Method::POST {
                if let Ok(req) = serde_json::from_slice::<Value>(body) {
                    let team = state::my_team_mut(&mut st);
                    let sq = if req.get("squad").is_some() {
                        req.get("squad").cloned().unwrap_or(json!({}))
                    } else {
                        req.clone()
                    };

                    let known_ids: std::collections::HashSet<String> = team
                        .get("players")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|p| {
                                    p.get("id")
                                        .or_else(|| p.get("_id"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    if let Some(pos_arr) = sq.get("positions").and_then(|v| v.as_array()) {
                        let valid_positions: Vec<Value> = pos_arr
                            .iter()
                            .filter(|p| {
                                let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
                                known_ids.contains(id)
                            })
                            .cloned()
                            .collect();

                        let starter_count = valid_positions
                            .iter()
                            .filter(|p| p.get("pos").and_then(|v| v.as_u64()).unwrap_or(99) < 28)
                            .count();
                        if starter_count >= 11 {
                            if let Some(sq_obj) = team.get_mut("squad").and_then(|v| v.as_object_mut()) {
                                sq_obj.insert("positions".to_string(), Value::Array(valid_positions));
                                if let Some(form) = sq.get("formation") {
                                    sq_obj.insert("formation".to_string(), form.clone());
                                }
                                if let Some(workrate) = sq.get("workrate") {
                                    sq_obj.insert("workrate".to_string(), workrate.clone());
                                }
                            }
                        }
                    }

                    let cap = req.get("captain").or_else(|| sq.get("captain"));
                    if let Some(c) = cap {
                        team["captain"] = c.clone();
                        if let Some(sq_obj) = team.get_mut("squad").and_then(|v| v.as_object_mut()) {
                            sq_obj.insert("captain".to_string(), c.clone());
                        }
                    }

                    let kickers = req.get("kickers").or_else(|| sq.get("kickers"));
                    if let Some(k) = kickers {
                        team["kickers"] = k.clone();
                        if let Some(sq_obj) = team.get_mut("squad").and_then(|v| v.as_object_mut()) {
                            sq_obj.insert("kickers".to_string(), k.clone());
                        }
                    }

                    let _ = state::save_state(&st);
                }
            }
            let body = build_coach_me(&mut st, slot);
            Some(json!({
                "result": true,
                "resources": [
                    {
                        "uri": "/coach/me",
                        "body": body
                    }
                ]
            }))
        }
        "/coach/0/default" | "/coach/1/default" => {
            let mut st = state::load_state();
            let slot: usize = if path.contains("/1/") { 1 } else { 0 };
            if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
                if let Some(c) = coaches.iter().find(|c| c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64)) {
                    if let Some(id) = c.get("_id").and_then(|v| v.as_str()) {
                        st["defaultcoachid"] = json!(id);
                    }
                }
            }
            let _ = state::daily_login(&mut st);
            let _ = state::save_state(&st);

            let body = build_coach_me(&mut st, slot);
            Some(json!({
                "result": true,
                "resources": [
                    {
                        "uri": "/coach/me",
                        "body": body
                    }
                ]
            }))
        }
        "/account" => {
            let mut acc: Value = from_responses_file("/account_base")
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_else(|| {
                    json!({
                        "result": true,
                        "option": {
                            "soundOption": {},
                            "screenOption": {"Quality": 0},
                            "aiOption": {},
                            "cameraOption": {}
                        },
                        "features": {}
                    })
                });

            let st = state::load_state();
            let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
            acc["result"] = json!(true);
            acc["account"] = json!({"uid": 1, "coachid": def_id});

            let mut opt = acc.get("option").cloned().unwrap_or_else(|| json!({}));
            if let Some(st_opt) = st.get("option").and_then(Value::as_object) {
                if let Some(opt_obj) = opt.as_object_mut() {
                    for (k, v) in st_opt {
                        opt_obj.insert(k.clone(), v.clone());
                    }
                }
            }

            // Sound defaults: Master, Commentary, PAVolume, PlayerCalls, SFX, Crowd, BGM
            let mut sound_map = serde_json::Map::new();
            for &(name, val) in &crate::http::constants::SOUND_DEFAULTS {
                sound_map.insert(name.to_string(), json!(val));
            }
            if let Some(existing_sound) = opt.get("soundOption").and_then(Value::as_object) {
                for (k, v) in existing_sound {
                    sound_map.insert(k.clone(), v.clone());
                }
            }
            opt["soundOption"] = Value::Object(sound_map);

            // Control defaults
            let ctl_ok = opt
                .get("controlOption")
                .map(crate::http::constants::is_valid_control_option)
                .unwrap_or(false);
            if !ctl_ok {
                opt["controlOption"] = crate::http::constants::control_defaults_json();
            }

            acc["option"] = opt.clone();
            acc["options"] = opt;
            Some(acc)
        }
        "/account/option" => {
            if method == Method::PUT || method == Method::POST {
                let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
                let mut st = state::load_state();
                let old_ctl = st.get("option").and_then(|o| o.get("controlOption")).cloned();
                let mut new_opt = serde_json::Map::new();
                if let Some(obj) = req.as_object() {
                    for (k, v) in obj {
                        if k.ends_with("Option") && k != "controlOption" {
                            new_opt.insert(k.clone(), v.clone());
                        }
                    }
                }
                let ctl = if let Some(ctl_val) = req.get("controlOption") {
                    if crate::http::constants::is_valid_control_option(ctl_val) {
                        Some(ctl_val.clone())
                    } else {
                        old_ctl
                    }
                } else {
                    old_ctl
                };
                if let Some(c) = ctl {
                    new_opt.insert("controlOption".to_string(), c);
                }
                st["option"] = Value::Object(new_opt);
                let _ = state::save_state(&st);
            }
            Some(json!({"result": true}))
        }
        "/info/server/2ndpass" => Some(json!({
            "result": true,
            "has2ndpass": false,
            "status": "ok"
        })),
        "/info/client/validation" => Some(json!({
            "result": true,
            "status": "ok"
        })),
        "/game/my/tactic/all" => {
            let st = state::load_state();
            let mut list = Vec::with_capacity(6);
            let saved = st.get("tactics").and_then(|t| t.as_object());
            for i in 0..6 {
                let tac = saved
                    .and_then(|s| s.get(&i.to_string()))
                    .cloned()
                    .unwrap_or_else(|| default_tactic(i));
                list.push(tac);
            }
            Some(Value::Array(list))
        }
        "/game/my/team/captain" => {
            let mut st = state::load_state();
            if method == Method::PUT || method == Method::POST {
                if let Ok(req) = serde_json::from_slice::<Value>(body) {
                    if let Some(id) = req.get("id").and_then(|v| v.as_str()) {
                        let team = state::my_team_mut(&mut st);
                        if let Some(sq) = team.get_mut("squad") {
                            sq["captain"] = json!(id);
                        }
                        let _ = state::save_state(&st);
                    }
                }
            }
            Some(json!({"result": true}))
        }
        "/events" => Some(json!({
            "events": [
                {
                    "eventid": 1001,
                    "active": true,
                    "reason": [],
                    "condition": {
                        "type": "enhance",
                        "enhance": {"season": []},
                        "timeset": {
                            "startdate": "20140101",
                            "enddate": "20351231",
                            "starttime": "0000",
                            "endtime": "2359"
                        },
                        "targetuser": {"targetuser_igr": 255, "targetuser_vip": 255}
                    },
                    "reward": [[{"boosts": {"BOOST_PLUS_ENHANCE": 10}}]],
                    "messages": {
                        "main": {
                            "name": "Event Upgrade",
                            "description": "Peluang sukses upgrade +10%."
                        }
                    }
                }
            ]
        })),
        p if p == "/shop/list" || p.starts_with("/shop/list/") => Some(json!([])),
        "/monthlypack" => {
            let st = state::load_state();
            if method == Method::POST {
                Some(json!({
                    "result": true,
                    "resources": [
                        {"uri": "/monthlypack", "body": []},
                        {"uri": "/bag", "body": state::my_coach(&st).get("bag").cloned().unwrap_or(json!([]))}
                    ]
                }))
            } else {
                Some(json!([]))
            }
        },
        "/specifictarget" => Some(json!({"issell": true, "targets": []})),
        "/bag" => {
            let st = state::load_state();
            let coach = state::my_coach(&st);
            Some(json!({
                "bag": coach.get("bag").cloned().unwrap_or(json!([]))
            }))
        }
        "/bag/unpackall" => Some(json!({"result": true})),
        "/static/info/teamcolor" => Some(json!({
            "teamColors": crate::http::team_color::get_all_team_colors(),
            "teamColorSpecials": {}
        })),
        "/static/db/STATICVALUES/max_coachlevel" => Some(json!(100)),
        "/account/trade2/usage" => Some(json!({"result": true, "usedcount": 0, "maxcount": 0, "remainsec": 0})),
        "/trade3/notification" => Some(json!({"result": true})),
        "/players/trade/proposal/resend" => Some(json!({"result": false, "reason": "NOT_SUPPORTED_REQUEST"})),
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
                        "inweburl" => Some(json!({"result": true, "url": "about:blank"})),
                        "bescript" => {
                            if let Some(data) = from_responses_file("/static/bescript/camera_controller") {
                                if let Ok(v) = serde_json::from_slice::<Value>(&data) {
                                    return Some(v);
                                }
                            }
                            Some(json!({}))
                        }
                        "tactic_slot" => {
                            let idx = caps.get(1).map(|m| m.as_str()).unwrap_or("0").parse::<usize>().unwrap_or(0);
                            let mut st = state::load_state();
                            if method == Method::PUT || method == Method::POST {
                                if let Ok(mut req) = serde_json::from_slice::<Value>(body) {
                                    if req.get("bus").is_some() {
                                        req["id"] = json!(idx);
                                        if st.get("tactics").is_none() {
                                            st["tactics"] = json!({});
                                        }
                                        st["tactics"][idx.to_string()] = req;
                                        let _ = state::save_state(&st);
                                        return Some(json!({"result": true}));
                                    }
                                }
                            }
                            Some(json!({"result": true, "resources": [{"uri": format!("/game/my/tactic/{}", idx), "body": default_tactic(idx)}]}))
                        }
                        "team_kickers" => {
                            let idx = caps.get(1).map(|m| m.as_str()).unwrap_or("0").parse::<usize>().unwrap_or(0);
                            let mut st = state::load_state();
                            if method == Method::PUT || method == Method::POST {
                                if let Ok(req) = serde_json::from_slice::<Value>(body) {
                                    if let Some(id) = req.get("id").and_then(|v| v.as_str()) {
                                        let team = state::my_team_mut(&mut st);
                                        if let Some(sq) = team.get_mut("squad") {
                                            let mut kickers = sq.get("kickers").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                                            while kickers.len() <= idx {
                                                kickers.push(json!(""));
                                            }
                                            kickers[idx] = json!(id);
                                            sq["kickers"] = Value::Array(kickers);
                                        }
                                        let _ = state::save_state(&st);
                                    }
                                }
                            }
                            Some(json!({"result": true}))
                        }
                        "coach_create" => {
                            let slot: usize = caps.get(1).map(|m| m.as_str()).unwrap_or("0").parse().unwrap_or(0);
                            let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
                            let mut st = state::load_state();
                            let teamid = req.get("id").and_then(|v| v.as_u64()).unwrap_or(1);
                            let coach_name = req.get("coachName").and_then(|v| v.as_str()).unwrap_or("Manager");
                            let cid = format!("{:024}", slot + 1);
                            let new_coach = json!({
                                "_id": cid,
                                "slot": slot,
                                "name": coach_name,
                                "state": "active",
                                "teamid": teamid,
                                "ep": state::START_EP,
                                "bag": [],
                                "items": [],
                                "daily": {"last": "", "streak": 0, "total": 0},
                                "scout_used": {}
                            });
                            let mut coaches = st.get("coaches").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                            coaches.retain(|c| c.get("slot").and_then(|v| v.as_u64()) != Some(slot as u64));
                            coaches.push(new_coach);
                            st["coaches"] = Value::Array(coaches);
                            if st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
                                st["defaultcoachid"] = json!(cid);
                            }
                            let _ = state::save_state(&st);
                            Some(json!({"result": true}))
                        }
                        "coach_new_team" => {
                            let slot: usize = caps.get(1).map(|m| m.as_str()).unwrap_or("0").parse().unwrap_or(0);
                            let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
                            let teamid = req.get("id").and_then(|v| v.as_u64()).unwrap_or(1);
                            let spids: Vec<u64> = req.get("spids")
                                .and_then(|v| v.as_array())
                                .map(|arr| arr.iter().filter_map(|x| x.as_u64()).collect())
                                .unwrap_or_default();

                            let mut team = from_responses_file(&format!("/static/info/authentic/team/all/{}", teamid))
                                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                                .and_then(|v| v.get("team").cloned())
                                .unwrap_or_else(|| json!({
                                    "teamid": teamid,
                                    "players": [],
                                    "squad": {"captain": "", "kickers": ["", "", "", "", ""], "positions": []}
                                }));

                            if !spids.is_empty() {
                                if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
                                    players.retain(|p| p.get("spid").and_then(|s| s.as_u64()).map(|s| spids.contains(&s)).unwrap_or(false));
                                }
                                if let Some(positions) = team.get_mut("squad").and_then(|s| s.get_mut("positions")).and_then(|v| v.as_array_mut()) {
                                    positions.retain(|p| p.get("spid").and_then(|s| s.as_u64()).map(|s| spids.contains(&s)).unwrap_or(false));
                                }
                            }

                            let mut st = state::load_state();
                            if st.get("teams").is_none() {
                                st["teams"] = json!({});
                            }
                            st["teams"][slot.to_string()] = team.clone();
                            let _ = state::save_state(&st);
                            Some(json!({"result": true, "team": team}))
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

    // 3. Crossdomain XML for Flash
    if path == "/crossdomain.xml" {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/xml")],
            "<?xml version=\"1.0\"?><!DOCTYPE cross-domain-policy SYSTEM \"http://www.adobe.com/xml/dtds/cross-domain-policy.dtd\"><cross-domain-policy><allow-access-from domain=\"*\" secure=\"false\"/><site-control permitted-cross-domain-policies=\"all\"/></cross-domain-policy>",
        )
            .into_response();
    }

    // 4. /cdn/ static asset serving
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

    // 5. Synchronize state operations
    let _lock = state::STATE_MUTEX.lock().unwrap();

    // 6. Static responses/<path>.json fallback
    if let Some(file_data) = from_responses_file(path) {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            file_data,
        )
            .into_response();
    }

    // 7. Exact endpoints or pattern routes
    if let Some(val) = handle_exact_or_pattern(&method, path, query, &body, host) {
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
