use chrono::{DateTime, Utc};
use rand::Rng;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

pub static STATE_MUTEX: Mutex<()> = Mutex::new(());

pub const STATE_PATH: &str = "gamedata/state.json";
pub const DAILY_FILE: &str = "gamedata/daily_rewards.json";
pub const FIFADB_PATH: &str = "gamedata/fifadb.sqlite";
pub const LOG_PATH: &str = "captures/http_config.log";

pub const START_EP: i64 = 1_000_000;
pub const VIP_UNTIL: i64 = 2_051_222_400_000; // ms: 2035-01-01
pub const MAX_PLAYERS: usize = 200;
pub const POS_SUB: u32 = 28;
pub const WIB: i64 = 7 * 3600;
pub const EP_ITEM: u64 = 1;

pub const GRADE_BOOST: [u32; 11] = [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24];
pub const GRADE_PRICE: [f64; 11] = [
    0.0, 1.0, 2.65, 7.1, 19.6, 53.0, 162.0, 420.0, 1090.0, 2680.0, 6290.0,
];

pub const POS_NAMES: [&str; 28] = [
    "GK", "SW", "RWB", "RB", "RCB", "CB", "LCB", "LB", "LWB", "RDM", "CDM", "LDM", "RM", "RCM",
    "CM", "LCM", "LM", "RAM", "CAM", "LAM", "RF", "CF", "LF", "RW", "RS", "ST", "LS", "LW",
];

pub const PACK_DEFS: &[(u64, &str, u64)] = &[
    (28001, "Premier League 65+", 150_000),
    (28002, "Bundesliga 65+", 140_000),
    (28003, "LaLiga 65+", 160_000),
    (28004, "Serie A 65+", 130_000),
    (28005, "Ligue 1 65+", 120_000),
    (28006, "FW 60+", 80_000),
    (28007, "MF 60+", 70_000),
    (28008, "DF 60+", 60_000),
    (28009, "GK 60+", 50_000),
    (8223600, "FW 65+", 180_000),
    (8223601, "MF 65+", 170_000),
    (8223602, "DF 65+", 150_000),
    (8223603, "GK 65+", 120_000),
    (8223508, "FW NEW 70+ 2017", 450_000),
    (8223509, "MF NEW 70+ 2017", 420_000),
    (8223510, "DF NEW 70+ 2017", 380_000),
    (82214068, "World Best 300 2017", 1_500_000),
    (82214180, "World Best 50 2017", 5_000_000),
    (45011, "This Season Top 10", 8_000_000),
    (45012, "This Season Top 20", 4_000_000),
    (45013, "This Season Top 30", 2_500_000),
    (45097, "This Season Top 30 +8", 50_000_000),
    (45004, "06-11 Player Top 10", 6_000_000),
    (45006, "06-11 Player Top 30", 2_000_000),
    (45007, "06-11 Player Top 50", 1_200_000),
    (82214356, "All '07", 500_000),
    (739, "'08 Top 3", 12_000_000),
    (15031, "'14 Top 30 +5", 15_000_000),
    (16001, "'15 Top 30 +5", 16_000_000),
    (7622, "World Cup '10 Top 30", 3_000_000),
    (7618, "Europe '10 Top 30 +3", 7_000_000),
    (880047, "World Cup '14 Top 50", 2_200_000),
    (45020, "Special Season '14 Top 30", 3_500_000),
    (45063, "Special Season '14 Top 50 +5", 18_000_000),
    (45026, "Team of the Season '14 Top 20", 4_500_000),
    (45027, "Team of the Season '14 Top 30", 3_200_000),
    (45028, "Team of the Season '14 Top 50", 2_000_000),
    (45130, "Team of the Season '14 All", 1_000_000),
    (8157, "EC Top 10", 9_000_000),
    (82232124, "EC Top 30 +3", 8_500_000),
    (8135, "Best EC 30 +5", 22_000_000),
];

pub fn get_pack_info(itemid: u64) -> Option<(&'static str, u64)> {
    PACK_DEFS
        .iter()
        .find(|(id, _, _)| *id == itemid)
        .map(|(_, name, price)| (*name, *price))
}

pub fn new_iid() -> String {
    let mut bytes = [0u8; 12];
    rand::thread_rng().fill(&mut bytes[..]);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn get_default_team(teamid: u64) -> Value {
    let candidate_paths = [
        format!("responses/static_info_authentic_team_all_{}.json", teamid),
        format!(r"C:\Users\ngnsusinn\Documents\fifa-online-3-server
esponses\static_info_authentic_team_all_{}.json", teamid),
    ];
    let mut team = json!({});
    for path in &candidate_paths {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Value>(&data) {
                if let Some(t) = v.get("team") {
                    team = t.clone();
                    break;
                }
            }
        }
    }

    if team.get("players").is_none() || team["players"].as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return json!({
            "teamid": teamid,
            "players": [],
            "squad": {"captain": "", "kickers": ["", "", "", "", ""], "positions": []},
            "captain": "",
            "kickers": ["", "", "", "", ""],
            "presetidx": 0,
            "formations": []
        });
    }

    let mut players = team.get("players").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for p in &mut players {
        let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        let id_str = spid.to_string();
        if let Some(obj) = p.as_object_mut() {
            obj.insert("_id".to_string(), json!(id_str));
            obj.insert("id".to_string(), json!(id_str));
            obj.insert("state".to_string(), json!("first"));
            obj.insert("grade".to_string(), json!(0));
            obj.insert("life".to_string(), json!(100));
            obj.insert("levelinfo".to_string(), json!({"level": 1, "exp": 0, "need": 100}));
            if !obj.contains_key("price") {
                obj.insert("price".to_string(), json!(10000));
            }
        }
    }

    let mut squad = team.get("squad").cloned().unwrap_or(json!({}));
    let mut cap_str = String::new();
    let mut kick_arr = Vec::new();

    if let Some(sq_obj) = squad.as_object_mut() {
        if let Some(positions) = sq_obj.get_mut("positions").and_then(|v| v.as_array_mut()) {
            for pos in positions {
                let spid = pos.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                if let Some(pos_obj) = pos.as_object_mut() {
                    pos_obj.insert("id".to_string(), json!(spid.to_string()));
                }
            }
        }
        if let Some(cap) = sq_obj.get("captain").and_then(|v| v.as_u64()) {
            cap_str = cap.to_string();
            sq_obj.insert("captain".to_string(), json!(cap_str));
        } else if let Some(cap) = sq_obj.get("captain").and_then(|v| v.as_str()) {
            cap_str = cap.to_string();
        }
        if let Some(kickers) = sq_obj.get_mut("kickers").and_then(|v| v.as_array_mut()) {
            for k in kickers {
                if let Some(kid) = k.as_u64() {
                    let s = kid.to_string();
                    kick_arr.push(json!(s.clone()));
                    *k = json!(s);
                } else if let Some(ks) = k.as_str() {
                    kick_arr.push(json!(ks));
                }
            }
        }
    }

    team["players"] = Value::Array(players);
    team["squad"] = squad;
    team["captain"] = json!(cap_str);
    team["kickers"] = Value::Array(kick_arr);
    team["presetidx"] = json!(0);
    team
}

pub fn default_state() -> Value {
    let coach_id = "000000000000000000000001";
    let default_team = get_default_team(1);
    json!({
        "coaches": [
            {
                "slot": 0,
                "_id": coach_id,
                "name": "Manager",
                "ep": START_EP,
                "teamid": 1,
                "items": [],
                "bag": [],
                "daily": {
                    "last": "",
                    "streak": 0,
                    "total": 0
                },
                "scout_used": {}
            }
        ],
        "defaultcoachid": coach_id,
        "teams": {
            "0": default_team
        },
        "cups": {}
    })
}

pub fn load_state() -> Value {
    if let Ok(content) = fs::read_to_string(STATE_PATH) {
        if let Ok(mut v) = serde_json::from_str::<Value>(&content) {
            let need_populate = v.get("teams")
                .and_then(|t| t.get("0"))
                .and_then(|t0| t0.get("players"))
                .and_then(|p| p.as_array())
                .map(|arr| arr.is_empty())
                .unwrap_or(true);
            if need_populate {
                let teamid = v.get("coaches")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|c0| c0.get("teamid"))
                    .and_then(|t| t.as_u64())
                    .unwrap_or(1);
                let default_team = get_default_team(teamid);
                if v.get("teams").is_none() {
                    v["teams"] = json!({});
                }
                v["teams"]["0"] = default_team;
                let _ = save_state(&v);
            }
            return v;
        }
    }
    default_state()
}

pub fn save_state(st: &Value) -> Result<(), std::io::Error> {
    if let Some(parent) = Path::new(STATE_PATH).parent() {
        fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string(st).map_err(|e| std::io::Error::other(e.to_string()))?;
    fs::write(STATE_PATH, s)
}

pub fn default_slot(st: &Value) -> usize {
    let def_id = st.get("defaultcoachid").and_then(|v| v.as_str()).unwrap_or("");
    if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
        for c in coaches {
            if c.get("_id").and_then(|v| v.as_str()) == Some(def_id) {
                return c.get("slot").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            }
        }
    }
    0
}

pub fn my_coach_mut(st: &mut Value) -> &mut Value {
    let slot = default_slot(st);
    if let Some(coaches) = st.get_mut("coaches").and_then(|v| v.as_array_mut()) {
        let idx = coaches
            .iter()
            .position(|c| c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64))
            .unwrap_or(0);
        if idx < coaches.len() {
            return &mut coaches[idx];
        }
    }
    panic!("invalid state: no coaches");
}

pub fn my_coach(st: &Value) -> &Value {
    let slot = default_slot(st);
    if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
        for c in coaches {
            if c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64) {
                return c;
            }
        }
    }
    &Value::Null
}

pub fn my_team_mut(st: &mut Value) -> &mut Value {
    let slot = default_slot(st).to_string();
    let teams = st.get_mut("teams").unwrap();
    if teams.get(&slot).is_none() {
        teams[slot.clone()] = json!({
            "players": [],
            "squad": {
                "positions": [],
                "kickers": ["", "", "", "", ""],
                "captain": ""
            }
        });
    }
    &mut teams[slot]
}

pub fn my_team(st: &Value) -> &Value {
    let slot = default_slot(st).to_string();
    st.get("teams")
        .and_then(|t| t.get(&slot))
        .unwrap_or(&Value::Null)
}

pub fn iid(p: &Value) -> String {
    if let Some(id) = p.get("_id").and_then(|v| v.as_str()) {
        return id.to_string();
    }
    if let Some(id) = p.get("id").and_then(|v| v.as_str()) {
        return id.to_string();
    }
    if let Some(spid) = p.get("spid").and_then(|v| v.as_u64()) {
        return spid.to_string();
    }
    new_iid()
}

pub fn grade_of(p: &Value) -> usize {
    p.get("grade")
        .and_then(|v| v.as_u64())
        .map(|g| (g as usize).min(10))
        .unwrap_or(0)
}

pub fn sig3(x: u64) -> u64 {
    if x < 1000 {
        return x;
    }
    let s = x.to_string();
    let p = 10u64.pow((s.len().saturating_sub(3)) as u32);
    ((x as f64 / p as f64).round() as u64) * p
}

pub fn price_formula(ovr: u32) -> u64 {
    let val = (2.53 + 0.1113 * (ovr as f64)).exp();
    sig3((val.max(1000.0)) as u64)
}

pub fn stat_price(ovr: u32) -> u64 {
    price_formula(ovr)
}

pub fn card_ovr(spid: u64) -> u32 {
    if Path::new(FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(FIFADB_PATH) {
            let stmt = conn
                .prepare("SELECT overallrating FROM players WHERE spid = ?1")
                .ok();
            if let Some(mut s) = stmt {
                if let Ok(ovr) = s.query_row([spid], |row| row.get::<_, u32>(0)) {
                    return ovr;
                }
            }
        }
    }
    75
}

pub fn card_price(spid: u64, grade: usize) -> u64 {
    let g = grade.min(10);
    let base = stat_price(card_ovr(spid)) as f64;
    sig3((base * GRADE_PRICE[g]) as u64)
}

pub fn names(spids: &[u64]) -> HashMap<u64, String> {
    let mut out = HashMap::new();
    if spids.is_empty() || !Path::new(FIFADB_PATH).exists() {
        return out;
    }
    if let Ok(conn) = Connection::open(FIFADB_PATH) {
        let placeholders = vec!["?"; spids.len()].join(",");
        let sql = format!(
            "SELECT p.spid, COALESCE(n.full_eng, n.full, n.short) \
             FROM players p JOIN nameseng n ON n.playerid = p.playerid \
             WHERE p.spid IN ({})",
            placeholders
        );
        let params: Vec<&dyn rusqlite::ToSql> =
            spids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        if let Ok(mut stmt) = conn.prepare(&sql) {
            if let Ok(rows) = stmt.query_map(&params[..], |row| {
                Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?))
            }) {
                for r in rows.flatten() {
                    out.insert(r.0, r.1);
                }
            }
        }
    }
    out
}

pub fn search_players(q: &str) -> Vec<Value> {
    let mut out = Vec::new();
    if q.len() < 2 || !Path::new(FIFADB_PATH).exists() {
        return out;
    }
    if let Ok(conn) = Connection::open(FIFADB_PATH) {
        let pattern = format!("%{}%", q);
        let sql = "SELECT p.spid, COALESCE(n.full_eng, n.full, n.short), p.overallrating, p.preferredposition1 \
                   FROM players p JOIN nameseng n ON n.playerid = p.playerid \
                   WHERE n.full LIKE ?1 OR n.short LIKE ?1 OR n.full_eng LIKE ?1 OR n.short_eng LIKE ?1 \
                   ORDER BY 3 DESC LIMIT 60";
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([&pattern], |row| {
                Ok((
                    row.get::<_, u64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            }) {
                for r in rows.flatten() {
                    let spid = r.0;
                    let name = r.1;
                    let ovr = r.2;
                    let pp = r.3 as usize;
                    let pos_str = if pp < POS_NAMES.len() {
                        POS_NAMES[pp]
                    } else {
                        "?"
                    };
                    out.push(json!({
                        "spid": spid,
                        "name": name,
                        "season": spid / 1_000_000,
                        "ovr": ovr,
                        "pos": pos_str,
                        "market": true,
                        "price": card_price(spid, 1)
                    }));
                }
            }
        }
    }
    out
}

pub fn in_squad(team: &Value, p: &Value) -> bool {
    let id_str = iid(p);
    if let Some(positions) = team
        .get("squad")
        .and_then(|sq| sq.get("positions"))
        .and_then(|pos| pos.as_array())
    {
        for s in positions {
            let pid = s
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| s.get("spid").and_then(|v| v.as_u64()).map(|n| n.to_string()))
                .unwrap_or_default();
            if pid == id_str {
                return true;
            }
        }
    }
    false
}

pub fn add_player(team: &mut Value, spid: u64, grade: u32) -> Value {
    let id = new_iid();
    let player = json!({
        "_id": id,
        "spid": spid,
        "expandme": "player",
        "grade": grade,
        "jersey": 99,
        "jerseyname": ""
    });
    if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
        players.push(player.clone());
    }
    player
}

pub fn remove_player(team: &mut Value, target_id: &str) -> bool {
    let mut removed = false;
    if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
        let before_len = players.len();
        players.retain(|p| iid(p) != target_id);
        removed = players.len() != before_len;
    }
    if let Some(sq) = team.get_mut("squad") {
        if let Some(positions) = sq.get_mut("positions").and_then(|v| v.as_array_mut()) {
            positions.retain(|s| {
                let pid = s
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| s.get("spid").and_then(|v| v.as_u64()).map(|n| n.to_string()))
                    .unwrap_or_default();
                pid != target_id
            });
        }
        if let Some(kickers) = sq.get_mut("kickers").and_then(|v| v.as_array_mut()) {
            for k in kickers {
                if k.as_str() == Some(target_id) {
                    *k = json!("");
                }
            }
        }
        if sq.get("captain").and_then(|v| v.as_str()) == Some(target_id) {
            sq["captain"] = json!("");
        }
    }
    removed
}

pub fn add_ep(st: &mut Value, amount: i64) {
    let coach = my_coach_mut(st);
    let cur = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(START_EP);
    coach["ep"] = json!(cur + amount);
}

pub fn give_pack(st: &mut Value, itemid: u64, count: u64) {
    let coach = my_coach_mut(st);
    if coach.get("items").is_none() {
        coach["items"] = json!([]);
    }
    let items = coach.get_mut("items").and_then(|v| v.as_array_mut()).unwrap();
    if let Some(stack) = items
        .iter_mut()
        .find(|i| i.get("itemid").and_then(|v| v.as_u64()) == Some(itemid))
    {
        let remains = stack.get("remains").and_then(|v| v.as_u64()).unwrap_or(0);
        stack["remains"] = json!(remains + count);
    } else {
        items.push(json!({
            "_id": new_iid(),
            "itemid": itemid,
            "remains": count
        }));
    }
}

pub fn today_wib(now_sec: Option<i64>) -> String {
    let ts = now_sec.unwrap_or_else(|| Utc::now().timestamp()) + WIB;
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "2026-01-01".to_string())
}

pub fn server_time_wib() -> String {
    let ts = Utc::now().timestamp() + WIB;
    DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S WIB").to_string())
        .unwrap_or_else(|| "2026-01-01 00:00:00 WIB".to_string())
}

pub fn w3c_time(sec: i64) -> String {
    DateTime::from_timestamp(sec, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S.000Z").to_string())
        .unwrap_or_default()
}

pub fn bag_add(
    st: &mut Value,
    title: &str,
    items: Vec<Value>,
    players: Vec<Value>,
    msg: &str,
) -> Value {
    let entry = json!({
        "_id": new_iid(),
        "title": title,
        "msg": msg,
        "items": items,
        "players": players,
        "createdat": Utc::now().timestamp()
    });
    let coach = my_coach_mut(st);
    if coach.get("bag").is_none() {
        coach["bag"] = json!([]);
    }
    coach["bag"].as_array_mut().unwrap().push(entry.clone());
    entry
}

pub fn daily_default_table() -> Vec<Value> {
    vec![
        json!({"ep": 10000}),
        json!({"ep": 20000}),
        json!({"pack": 28007}),
        json!({"ep": 30000}),
        json!({"pack": 28006}),
        json!({"ep": 50000}),
        json!({"ep": 100000, "pack": 82214068}),
    ]
}

pub fn daily_table() -> Vec<Value> {
    if let Ok(content) = fs::read_to_string(DAILY_FILE) {
        if let Ok(t) = serde_json::from_str::<Vec<Value>>(&content) {
            if !t.is_empty() {
                return t;
            }
        }
    }
    daily_default_table()
}

pub fn reward_items(reward: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(ep) = reward.get("ep").and_then(|v| v.as_i64()) {
        if ep > 0 {
            out.push(json!({"itemid": EP_ITEM, "count": ep}));
        }
    }
    if let Some(pack) = reward.get("pack").and_then(|v| v.as_u64()) {
        let count = reward.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
        out.push(json!({"itemid": pack, "count": count}));
    }
    out
}

pub fn daily_login(st: &mut Value) -> Option<Value> {
    let now = Utc::now().timestamp();
    let today = today_wib(Some(now));
    let yesterday = today_wib(Some(now - 86400));
    let coach = my_coach_mut(st);
    if coach.get("daily").is_none() {
        coach["daily"] = json!({"last": "", "streak": 0, "total": 0});
    }
    let last = coach["daily"]
        .get("last")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if last == today {
        return None;
    }
    let streak = if last == yesterday {
        coach["daily"].get("streak").and_then(|v| v.as_u64()).unwrap_or(0) + 1
    } else {
        1
    };
    let total = coach["daily"].get("total").and_then(|v| v.as_u64()).unwrap_or(0) + 1;
    coach["daily"]["last"] = json!(today);
    coach["daily"]["streak"] = json!(streak);
    coach["daily"]["total"] = json!(total);

    let table = daily_table();
    let idx = ((streak - 1) as usize) % table.len();
    let rew = &table[idx];
    let items = reward_items(rew);
    let title = format!("Quà đăng nhập hàng ngày - Ngày {}", streak);
    let msg = format!(
        "Đăng nhập {} ngày liên tục. Đăng nhập tiếp vào ngày mai để nhận quà ngày {}.",
        streak,
        streak + 1
    );

    let entry = json!({
        "_id": new_iid(),
        "title": title,
        "msg": msg,
        "items": items,
        "players": [],
        "createdat": now
    });
    if coach.get("bag").is_none() {
        coach["bag"] = json!([]);
    }
    coach["bag"].as_array_mut().unwrap().push(entry.clone());
    Some(entry)
}

pub fn coach_scouters(st: &Value, _slot: usize) -> Vec<Value> {
    let now = Utc::now().timestamp() as f64;
    let coach = my_coach(st);
    let used = coach.get("scout_used").and_then(|v| v.as_object());

    let scouts_def = [
        ("free", "STR_SCOUTER_KR_FREE_A01", 0.0, None),
        ("silver", "STR_SCOUTER_KR_SPECIAL_A01", 10800.0, Some(json!({"require_grade": 2}))),
        ("gold", "STR_SCOUTER_KR_SPECIAL_B01", 21600.0, Some(json!({"require_grade": 3}))),
        ("event", "STR_SCOUTER_KR_EVENT_A01", 0.0, Some(json!({"require_season": "47,69,77,53,35,23,91,93", "require_season_count": 2}))),
        ("vvip", "STR_SCOUTER_KR_VVIP_A01", 0.0, Some(json!({"require_value": 1_000_000_000}))),
    ];

    let mut out = Vec::new();
    for (key, sid, cool, quest) in scouts_def {
        let last_used = used
            .and_then(|m| m.get(key))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let until = last_used + cool;
        let is_locked = until > now;
        let mut obj = json!({
            "key": key,
            "scoutid": key,
            "scoutername": format!("{}_NAME", sid),
            "scouterdesc": format!("{}_DESC", sid),
            "lock": is_locked
        });
        if is_locked {
            obj["cooltime"] = json!(w3c_time(until as i64));
        }
        if let Some(q) = quest {
            obj["quest"] = q;
        }
        out.push(obj);
    }
    out
}
