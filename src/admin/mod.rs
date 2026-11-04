use crate::state;
use axum::{
    body::Bytes,
    extract::ConnectInfo,
    http::{header, HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Router,
};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

const ADMIN_PAGE_PATH: &str = "admin/index.html";

fn as_int(v: &Value, lo: i64, hi: i64, what: &str) -> Result<i64, String> {
    let num = if let Some(n) = v.as_i64() {
        n
    } else if let Some(f) = v.as_f64() {
        f as i64
    } else if let Some(s) = v.as_str() {
        s.trim()
            .parse::<i64>()
            .map_err(|_| format!("{} phải là số", what))?
    } else {
        return Err(format!("{} phải là số", what));
    };
    if num < lo || num > hi {
        return Err(format!("{} phải từ {}..{}", what, lo, hi));
    }
    Ok(num)
}

fn handle_summary(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let st = state::load_state();
    let coach = state::my_coach(&st);
    let slot = state::default_slot(&st);
    let default_daily = json!({"last": "", "streak": 0, "total": 0});
    let d = coach.get("daily").unwrap_or(&default_daily);
    let team = state::my_team(&st);
    let players_count = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let packs_count: u64 = coach
        .get("items")
        .and_then(|i| i.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|it| it.get("remains").and_then(|r| r.as_u64()))
                .sum()
        })
        .unwrap_or(0);
    let bag_count = coach
        .get("bag")
        .and_then(|b| b.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let today = state::today_wib(None);
    let granted_today = d.get("last").and_then(|v| v.as_str()) == Some(&today);

    Ok(json!({
        "coach": {
            "name": coach.get("name").cloned().unwrap_or(json!("Manager")),
            "ep": coach.get("ep").cloned().unwrap_or(json!(state::START_EP)),
            "teamid": coach.get("teamid").cloned().unwrap_or(json!(1))
        },
        "vip_until": "2035-01-01",
        "igr": true,
        "players": players_count,
        "max_players": state::MAX_PLAYERS,
        "packs": packs_count,
        "bag": bag_count,
        "daily": {
            "last": d.get("last").unwrap_or(&json!("")),
            "streak": d.get("streak").unwrap_or(&json!(0)),
            "total": d.get("total").unwrap_or(&json!(0)),
            "today": today,
            "granted_today": granted_today
        },
        "scouts": state::coach_scouters(&st, slot),
        "server_time": state::server_time_wib()
    }))
}

fn handle_players(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let st = state::load_state();
    let team = state::my_team(&st);
    let empty_vec = vec![];
    let players = team
        .get("players")
        .and_then(|p| p.as_array())
        .unwrap_or(&empty_vec);
    let positions = team
        .get("squad")
        .and_then(|sq| sq.get("positions"))
        .and_then(|pos| pos.as_array())
        .unwrap_or(&empty_vec);

    let mut pos_map: HashMap<String, u32> = HashMap::new();
    for s in positions {
        let id = s
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| s.get("spid").and_then(|v| v.as_u64()).map(|n| n.to_string()))
            .unwrap_or_default();
        let pos = s.get("pos").and_then(|v| v.as_u64()).unwrap_or(99) as u32;
        pos_map.insert(id, pos);
    }

    let spids: Vec<u64> = players
        .iter()
        .filter_map(|p| p.get("spid").and_then(|v| v.as_u64()))
        .collect();
    let name_map = state::names(&spids);

    let mut out = Vec::new();
    for x in players {
        let id = state::iid(x);
        let spid = x.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        let g = state::grade_of(x);
        let name = name_map.get(&spid).cloned().unwrap_or_else(|| "?".to_string());
        let ovr = state::card_ovr(spid) + state::GRADE_BOOST[g];
        let price = state::card_price(spid, g);
        let role = match pos_map.get(&id) {
            Some(&p) if p < state::POS_SUB => "MAIN",
            Some(_) => "SUB",
            None => "RES",
        };
        let team2 = x.get("team2").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(json!({
            "id": id,
            "spid": spid,
            "name": name,
            "season": spid / 1_000_000,
            "ovr": ovr,
            "grade": g,
            "price": price,
            "role": role,
            "team2": team2
        }));
    }
    Ok(json!({"players": out}))
}

fn handle_search(params: &HashMap<String, Value>) -> Result<Value, String> {
    let q = params
        .get("q")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if q.trim().len() < 2 {
        return Ok(json!({"rows": []}));
    }
    let rows = state::search_players(q.trim());
    Ok(json!({"rows": rows}))
}

fn handle_packs(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let packs: Vec<Value> = state::PACK_DEFS
        .iter()
        .map(|(id, name, price)| {
            json!({
                "itemid": id,
                "name": name,
                "price": price
            })
        })
        .collect();
    Ok(json!({"packs": packs}))
}

fn handle_ep(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let direct = params.get("direct").and_then(|v| v.as_bool()).unwrap_or(false);
    if let Some(set_val) = params.get("set") {
        let n = as_int(set_val, 0, 10_000_000_000_000, "EP")?;
        let coach = state::my_coach_mut(&mut st);
        coach["ep"] = json!(n);
    } else if let Some(add_val) = params.get("add") {
        let n = as_int(add_val, -10_000_000_000_000, 10_000_000_000_000, "EP")?;
        if n > 0 && !direct {
            let title = "Quà tặng EP";
            let msg = format!("Quà tặng {} EP từ ban quản trị máy chủ.", state::format_number(n as u64));
            state::bag_add(
                &mut st,
                title,
                vec![json!({"itemid": state::EP_ITEM, "count": n})],
                vec![],
                &msg,
            );
        } else {
            state::add_ep(&mut st, n);
            let coach = state::my_coach_mut(&mut st);
            let cur = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(0);
            if cur < 0 {
                coach["ep"] = json!(0);
            }
        }
    } else {
        return Err("Cần tham số EP add hoặc set".to_string());
    }
    let ep = state::my_coach(&st)
        .get("ep")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ep": ep, "ok": true}))
}

fn handle_player_add(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let spid = as_int(
        params.get("spid").ok_or("Cần có spid")?,
        1,
        1_000_000_000,
        "spid",
    )? as u64;
    let grade = params
        .get("grade")
        .map(|g| as_int(g, 1, 10, "cấp thẻ"))
        .transpose()?
        .unwrap_or(1) as u32;

    let names = state::names(&[spid]);
    if names.is_empty() && Path::new(state::FIFADB_PATH).exists() {
        return Err("spid không tồn tại trong cơ sở dữ liệu".to_string());
    }

    let team = state::my_team_mut(&mut st);
    let p_count = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if p_count >= state::MAX_PLAYERS {
        return Err(format!("CLB đã đầy (tối đa {} cầu thủ)", state::MAX_PLAYERS));
    }

    let p = state::add_player(team, spid, grade);
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"id": state::iid(&p)}))
}

fn handle_player_grade(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Cần có id")?;
    let grade = as_int(
        params.get("grade").ok_or("Cần có cấp thẻ")?,
        0,
        10,
        "cấp thẻ",
    )?;
    let team = state::my_team_mut(&mut st);
    let mut found = false;
    if let Some(players) = team.get_mut("players").and_then(|p| p.as_array_mut()) {
        for p in players {
            if state::iid(p) == id {
                p["grade"] = json!(grade);
                found = true;
                break;
            }
        }
    }
    if !found {
        return Err("Không tìm thấy cầu thủ".to_string());
    }
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_player_remove(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Cần có id")?;
    let team = state::my_team(&st);
    let p_opt = team
        .get("players")
        .and_then(|p| p.as_array())
        .and_then(|a| a.iter().find(|p| state::iid(p) == id));
    let p = match p_opt {
        Some(p) => p.clone(),
        None => return Err("Không tìm thấy cầu thủ".to_string()),
    };
    if state::in_squad(team, &p) {
        return Err("Cầu thủ đang ở trong đội hình 18 người; hãy thay ra ở mục Đội hình trước".to_string());
    }
    let team_mut = state::my_team_mut(&mut st);
    state::remove_player(team_mut, id);
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_player_team(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("Cần có id")?;
    let team_str = params.get("team").and_then(|v| v.as_str()).unwrap_or("");
    let team = state::my_team(&st);
    let p_opt = team
        .get("players")
        .and_then(|p| p.as_array())
        .and_then(|a| a.iter().find(|p| state::iid(p) == id));
    let p = match p_opt {
        Some(p) => p.clone(),
        None => return Err("Không tìm thấy cầu thủ".to_string()),
    };
    if team_str == "second" && state::in_squad(team, &p) {
        return Err("Cầu thủ thuộc đội hình 18 người phải ở Đội chính".to_string());
    }
    let team_mut = state::my_team_mut(&mut st);
    if let Some(players) = team_mut.get_mut("players").and_then(|p| p.as_array_mut()) {
        for x in players {
            if state::iid(x) == id {
                if team_str == "second" {
                    x["team2"] = json!(true);
                } else if let Some(obj) = x.as_object_mut() {
                    obj.remove("team2");
                }
                break;
            }
        }
    }
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_give_pack(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let itemid = as_int(
        params.get("itemid").ok_or("Cần có itemid")?,
        1,
        10_000_000_000,
        "gói thẻ",
    )? as u64;
    let pack_info = state::get_pack_info(itemid).ok_or("Gói thẻ không hợp lệ")?;
    let count = params
        .get("count")
        .map(|c| as_int(c, 1, 999, "số lượng"))
        .transpose()?
        .unwrap_or(1) as u64;

    let direct = params.get("direct").and_then(|v| v.as_bool()).unwrap_or(false);
    if direct {
        state::give_pack(&mut st, itemid, count);
    } else {
        let title = format!("Gói thẻ: {}", pack_info.0);
        let msg = format!("Quà tặng {}× {} từ ban quản trị máy chủ.", count, pack_info.0);
        state::bag_add(
            &mut st,
            &title,
            vec![json!({"itemid": itemid, "count": count})],
            vec![],
            &msg,
        );
    }
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_bag_send(params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let ep = params
        .get("ep")
        .map(|v| as_int(v, 0, 1_000_000_000_000, "EP"))
        .transpose()?
        .unwrap_or(0);
    let pack = params
        .get("pack")
        .and_then(|v| if v.is_null() { None } else { v.as_u64() });
    let count = params
        .get("count")
        .map(|v| as_int(v, 1, 999, "số lượng"))
        .transpose()?
        .unwrap_or(1) as u64;
    if let Some(pid) = pack {
        if state::get_pack_info(pid).is_none() {
            return Err("Gói thẻ không hợp lệ".to_string());
        }
    }
    let mut items = Vec::new();
    if ep > 0 {
        items.push(json!({"itemid": state::EP_ITEM, "count": ep}));
    }
    if let Some(pid) = pack {
        items.push(json!({"itemid": pid, "count": count}));
    }

    let mut players = Vec::new();
    if let Some(spid_val) = params.get("spid").filter(|v| !v.is_null()) {
        let spid = as_int(spid_val, 1, 1_000_000_000, "spid")? as u64;
        let grade = params
            .get("grade")
            .map(|v| as_int(v, 1, 10, "cấp thẻ"))
            .transpose()?
            .unwrap_or(1);
        let names = state::names(&[spid]);
        if names.is_empty() && Path::new(state::FIFADB_PATH).exists() {
            return Err("spid không tồn tại trong cơ sở dữ liệu".to_string());
        }
        players.push(json!({"spid": spid, "grade": grade}));
    }

    if items.is_empty() && players.is_empty() {
        return Err("Vui lòng nhập EP, gói thẻ hoặc cầu thủ".to_string());
    }
    let title = params
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Quà từ Admin");
    let title_sub: String = title.chars().take(60).collect();
    let e = state::bag_add(
        &mut st,
        &title_sub,
        items,
        players,
        "Quà tặng từ quản trị máy chủ.",
    );
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"id": e["_id"]}))
}

fn handle_daily(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let st = state::load_state();
    let coach = state::my_coach(&st);
    let default_daily = json!({"last": "", "streak": 0, "total": 0});
    let status = coach.get("daily").unwrap_or(&default_daily);
    Ok(json!({
        "table": state::daily_table(),
        "status": status,
        "today": state::today_wib(None),
        "file": state::DAILY_FILE
    }))
}

fn handle_daily_set(params: &HashMap<String, Value>) -> Result<Value, String> {
    let table = params
        .get("table")
        .and_then(|v| v.as_array())
        .ok_or("Cần có danh sách table")?;
    if table.is_empty() || table.len() > 31 {
        return Err("Bảng thưởng phải từ 1..31 ngày".to_string());
    }
    let mut sanitized = Vec::new();
    for (i, row) in table.iter().enumerate() {
        let day_num = i + 1;
        let ep = row
            .get("ep")
            .map(|v| as_int(v, 0, 1_000_000_000_000, &format!("EP ngày {}", day_num)))
            .transpose()?
            .unwrap_or(0);
        let pack = row
            .get("pack")
            .and_then(|v| if v.is_null() { None } else { Some(v) });
        let pack_id = if let Some(p) = pack {
            let pid = as_int(p, 1, 10_000_000_000, &format!("gói thẻ ngày {}", day_num))? as u64;
            if state::get_pack_info(pid).is_none() {
                return Err(format!("Gói thẻ ngày {} không hợp lệ", day_num));
            }
            Some(pid)
        } else {
            None
        };
        if ep == 0 && pack_id.is_none() {
            return Err(format!("Ngày {} chưa có phần thưởng", day_num));
        }
        let mut obj = serde_json::Map::new();
        if ep > 0 {
            obj.insert("ep".to_string(), json!(ep));
        }
        if let Some(pid) = pack_id {
            let count = row
                .get("count")
                .map(|v| as_int(v, 1, 99, "số lượng"))
                .transpose()?
                .unwrap_or(1);
            obj.insert("pack".to_string(), json!(pid));
            obj.insert("count".to_string(), json!(count));
        }
        sanitized.push(Value::Object(obj));
    }
    if let Some(parent) = Path::new(state::DAILY_FILE).parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(&sanitized).map_err(|e| e.to_string())?;
    fs::write(state::DAILY_FILE, content).map_err(|e| e.to_string())?;
    Ok(json!({"table": sanitized}))
}

fn handle_daily_reset(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let coach = state::my_coach_mut(&mut st);
    coach["daily"] = json!({"last": "", "streak": 0, "total": 0});
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_daily_grant(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let today = state::today_wib(None);
    let coach = state::my_coach_mut(&mut st);
    if coach.get("daily").is_none() {
        coach["daily"] = json!({"last": "", "streak": 0, "total": 0});
    }
    let last = coach["daily"]
        .get("last")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if last == today {
        let yesterday = state::today_wib(Some(Utc::now().timestamp() - 86400));
        coach["daily"]["last"] = json!(yesterday);
    }
    let e = state::daily_login(&mut st);
    state::save_state(&st).map_err(|e| e.to_string())?;
    let title = e
        .as_ref()
        .and_then(|entry| entry.get("title").and_then(|v| v.as_str()));
    Ok(json!({"title": title}))
}

fn handle_scouts_reset(_params: &HashMap<String, Value>) -> Result<Value, String> {
    let mut st = state::load_state();
    let coach = state::my_coach_mut(&mut st);
    coach["scout_used"] = json!({});
    state::save_state(&st).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

fn handle_log(params: &HashMap<String, Value>) -> Result<Value, String> {
    let n = params
        .get("n")
        .map(|v| as_int(v, 1, 1000, "n"))
        .transpose()?
        .unwrap_or(80) as usize;
    let mut lines = Vec::new();
    if let Ok(content) = fs::read_to_string(state::LOG_PATH) {
        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(n);
        for line in &all_lines[start..] {
            let s: String = line.chars().take(300).collect();
            lines.push(s);
        }
    }
    Ok(json!({"lines": lines}))
}

pub async fn admin_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // 1. IP check: remote must be loopback
    if !addr.ip().is_loopback() {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    // 2. Host check: Host header must be 127.0.0.1, localhost, or [::1] (ignoring port)
    let host_hdr = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let host = host_hdr.split(':').next().unwrap_or("");
    if host != "127.0.0.1" && host != "localhost" && host != "[::1]" && host != "::1" {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    let path = uri.path();

    // 3. Serve admin/index.html
    if path == "/" || path == "/index.html" {
        let page = fs::read(ADMIN_PAGE_PATH).unwrap_or_else(|_| b"Admin page not found".to_vec());
        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            page,
        )
            .into_response();
    }

    // 4. API endpoint routing
    if !path.starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            json!({"error": "not found"}).to_string(),
        )
            .into_response();
    }

    let name = &path[len("/api/")..];
    let read_only: HashSet<&str> = ["summary", "players", "search", "packs", "daily", "log"]
        .into_iter()
        .collect();

    // Check mutation security: POST + X-FO3-Admin: 1
    if !read_only.contains(name) {
        let admin_hdr = headers.get("X-FO3-Admin").and_then(|v| v.to_str().ok());
        if method != Method::POST || admin_hdr != Some("1") {
            return (
                StatusCode::FORBIDDEN,
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                json!({"error": "changes need POST with the X-FO3-Admin header"}).to_string(),
            )
                .into_response();
        }
    }

    // Parse params: query string + body JSON
    let mut params = HashMap::new();
    if let Some(q) = uri.query() {
        for pair in q.split('&') {
            let mut parts = pair.splitn(2, '=');
            if let Some(k) = parts.next() {
                let v = parts.next().unwrap_or("");
                let decoded_k = urlencoding_decode(k);
                let decoded_v = urlencoding_decode(v);
                params.insert(decoded_k, Value::String(decoded_v));
            }
        }
    }
    if !body.is_empty() {
        if let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(&body) {
            for (k, v) in map {
                params.insert(k, v);
            }
        }
    }

    // Synchronize with state lock
    let _lock = state::STATE_MUTEX.lock().unwrap();

    let res = match name {
        "summary" => handle_summary(&params),
        "players" => handle_players(&params),
        "search" => handle_search(&params),
        "packs" => handle_packs(&params),
        "ep" => handle_ep(&params),
        "player_add" => handle_player_add(&params),
        "player_grade" => handle_player_grade(&params),
        "player_remove" => handle_player_remove(&params),
        "player_team" => handle_player_team(&params),
        "give_pack" => handle_give_pack(&params),
        "bag_send" => handle_bag_send(&params),
        "daily" => handle_daily(&params),
        "daily_set" => handle_daily_set(&params),
        "daily_reset" => handle_daily_reset(&params),
        "daily_grant" => handle_daily_grant(&params),
        "scouts_reset" => handle_scouts_reset(&params),
        "log" => handle_log(&params),
        _ => {
            return (
                StatusCode::NOT_FOUND,
                [
                    (header::CONTENT_TYPE, "application/json"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                json!({"error": "not found"}).to_string(),
            )
                .into_response();
        }
    };

    let body_out = match res {
        Ok(v) => v,
        Err(e) => json!({"error": e}),
    };

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body_out.to_string(),
    )
        .into_response()
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(h) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(h);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

const fn len(s: &str) -> usize {
    s.len()
}

pub fn create_admin_router() -> Router {
    Router::new().fallback(admin_handler)
}

pub async fn run_admin_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let app = create_admin_router();
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("[admin] admin page on http://127.0.0.1:{}/", port);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
