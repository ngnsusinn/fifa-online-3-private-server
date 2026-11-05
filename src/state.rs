use chrono::{DateTime, Utc};
use rand::Rng;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
pub const ES_ITEM: u64 = 2;
pub const PACK_BUNCHES: &[u64] = &[1, 10];

pub const GRADE_BOOST: [u32; 11] = [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24];
pub const GRADE_PRICE: [f64; 11] = [
    0.0, 1.0, 2.65, 7.1, 19.6, 53.0, 162.0, 420.0, 1090.0, 2680.0, 6290.0,
];

pub const PLAYER_MAX_LEVEL: u32 = 23;
pub const COACH_MAX_LEVEL: u32 = 100;
pub const EXP_WIN: (u64, u64) = (300, 150);
pub const EXP_DRAW: (u64, u64) = (200, 100);
pub const EXP_LOSS: (u64, u64) = (100, 60);

pub fn player_need(level: u32) -> u64 {
    100 * (level as u64)
}

pub fn coach_need(level: u32) -> u64 {
    500 + 100 * (level.max(1) as u64 - 1)
}

pub fn coach_cumulative_exp(lvl: u32) -> u64 {
    (1..=lvl).map(coach_need).sum()
}

pub fn make_levelinfo(info: Option<&Value>, need: impl Fn(u32) -> u64, top: u32) -> Value {
    let raw_level = info
        .and_then(|i| i.get("level"))
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(1) as u32;
    let level = raw_level.clamp(1, top);
    let exp = if level >= top {
        0
    } else {
        info.and_then(|i| i.get("exp"))
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(0)
    };
    json!({
        "level": level,
        "exp": exp,
        "to": need(level),
        "need": need(level)
    })
}

pub fn player_levelinfo(p: &Value) -> Value {
    make_levelinfo(p.get("levelinfo"), player_need, PLAYER_MAX_LEVEL)
}

pub fn coach_levelinfo(coach: &Value) -> Value {
    make_levelinfo(coach.get("levelinfo"), coach_need, COACH_MAX_LEVEL)
}

pub fn gain_exp(owner: &mut Value, n: u64, need: impl Fn(u32) -> u64, top: u32) -> (Value, Value) {
    let prev = make_levelinfo(owner.get("levelinfo"), &need, top);
    let mut level = prev["level"].as_u64().unwrap_or(1) as u32;
    let mut exp = prev["exp"].as_u64().unwrap_or(0) + n;

    while level < top && exp >= need(level) {
        exp -= need(level);
        level += 1;
    }
    if level >= top {
        exp = 0;
    }

    let cur = json!({
        "level": level,
        "exp": exp,
        "to": need(level),
        "need": need(level)
    });

    if let Some(obj) = owner.as_object_mut() {
        obj.insert("levelinfo".to_string(), cur.clone());
    }

    (prev, cur)
}

pub fn gain_player_exp(player: &mut Value, n: u64) -> (Value, Value) {
    gain_exp(player, n, player_need, PLAYER_MAX_LEVEL)
}

pub fn gain_coach_exp(coach: &mut Value, n: u64) -> (Value, Value) {
    gain_exp(coach, n, coach_need, COACH_MAX_LEVEL)
}

pub fn gain_team_player_exp(team: &mut Value, target_id: &str, n: u64) -> Option<(Value, Value)> {
    if let Some(players) = team.get_mut("players").and_then(|p| p.as_array_mut()) {
        for p in players {
            if iid(p) == target_id {
                return Some(gain_player_exp(p, n));
            }
        }
    }
    None
}

pub fn get_expplayer() -> Value {
    let mut map = serde_json::Map::new();
    for lv in 1..=(PLAYER_MAX_LEVEL + 1) {
        map.insert(
            lv.to_string(),
            json!({
                "requiredplayerexp": player_need(lv)
            }),
        );
    }
    Value::Object(map)
}

pub fn get_expcoach() -> Value {
    let mut map = serde_json::Map::new();
    for lv in 1..=COACH_MAX_LEVEL {
        map.insert(
            lv.to_string(),
            json!({
                "totalexp": coach_cumulative_exp(lv)
            }),
        );
    }
    Value::Object(map)
}

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
    (8334, "World Best 5", 15_000_000),
    (8335, "World Legend Top 3", 25_000_000),
    (8194, "World Legend Top 30", 18_000_000),
    (82214007, "Top 300", 2_000_000),
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

pub fn default_staffroles() -> Value {
    let mut roles = Vec::with_capacity(7);
    for i in 1..=7 {
        roles.push(json!({
            "roleidx": 1,
            "slotidx": i,
            "open": true,
            "staffid": Value::Null
        }));
    }
    Value::Array(roles)
}

pub fn get_formations() -> Value {
    let candidate_paths = [
        PathBuf::from("gamedata/formations.json"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("gamedata/formations.json"),
    ];
    for p in &candidate_paths {
        if p.is_file() {
            if let Ok(content) = fs::read_to_string(p) {
                if let Ok(v) = serde_json::from_str::<Value>(&content) {
                    if let Some(common) = v.get("common").cloned() {
                        if common.as_array().map(|a| !a.is_empty()).unwrap_or(false) {
                            return common;
                        }
                    }
                }
            }
        }
    }
    json!([
        {
            "formationid": 1,
            "formationname": "4-4-2",
            "positions": [0, 3, 4, 6, 7, 12, 13, 15, 16, 24, 26],
            "offsets": [
                {"x": 0.5, "y": 0.02},
                {"x": 0.85, "y": 0.22},
                {"x": 0.62, "y": 0.16},
                {"x": 0.38, "y": 0.16},
                {"x": 0.15, "y": 0.22},
                {"x": 0.85, "y": 0.5},
                {"x": 0.62, "y": 0.46},
                {"x": 0.38, "y": 0.46},
                {"x": 0.15, "y": 0.5},
                {"x": 0.62, "y": 0.8},
                {"x": 0.38, "y": 0.8}
            ]
        }
    ])
}

pub fn fill_squad(sq: &mut Value, players: &[Value]) {
    let default_offsets: &[(u32, f64, f64)] = &[
        (0, 0.5, 0.02),   // GK
        (3, 0.85, 0.22),  // RB
        (4, 0.62, 0.16),  // RCB
        (6, 0.38, 0.16),  // LCB
        (7, 0.15, 0.22),  // LB
        (12, 0.85, 0.5),  // RM
        (13, 0.62, 0.46), // RCM
        (15, 0.38, 0.46), // LCM
        (16, 0.15, 0.5),  // LM
        (24, 0.62, 0.8),  // RS
        (26, 0.38, 0.8),  // LS
    ];

    let sq_obj = match sq.as_object_mut() {
        Some(o) => o,
        None => return,
    };

    if sq_obj.get("formation").is_none() || sq_obj["formation"] == json!(null) {
        sq_obj.insert("formation".to_string(), json!(23)); // 4-4-2(1)
    }

    let mut pos_arr = sq_obj
        .get("positions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Ensure all existing entries have "id" string
    for p in &mut pos_arr {
        let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        if p.get("id").is_none() || p["id"].as_str().map(|s| s.is_empty()).unwrap_or(true) {
            p["id"] = json!(spid.to_string());
        }
    }

    let taken_spids: std::collections::HashSet<u64> = pos_arr
        .iter()
        .filter_map(|p| p.get("spid").and_then(|v| v.as_u64()).map(|s| s % 1_000_000))
        .collect();

    // Find used starter positions (< 28)
    let used_slots: std::collections::HashSet<u32> = pos_arr
        .iter()
        .filter_map(|p| p.get("pos").and_then(|v| v.as_u64()).map(|pos| pos as u32))
        .filter(|&pos| pos < POS_SUB)
        .collect();

    let mut available_players: Vec<&Value> = players
        .iter()
        .filter(|p| {
            let base_spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0) % 1_000_000;
            !taken_spids.contains(&base_spid)
        })
        .collect();

    // Fill missing starter slots
    for &(slot_pos, _, _) in default_offsets {
        if !used_slots.contains(&slot_pos) {
            if let Some(p) = available_players.pop() {
                let pid = iid(p);
                let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                pos_arr.push(json!({
                    "id": pid,
                    "spid": spid,
                    "pos": slot_pos
                }));
            }
        }
    }

    // Assign starting [1, 1, x, y] to all starters (< 28)
    for p in &mut pos_arr {
        let pos_val = p.get("pos").and_then(|v| v.as_u64()).unwrap_or(99) as u32;
        if pos_val < POS_SUB {
            let (x, y) = default_offsets
                .iter()
                .find(|&&(pp, _, _)| pp == pos_val)
                .map(|&(_, x, y)| (x, y))
                .unwrap_or((0.5, 0.5));
            p["starting"] = json!([1, 1, x, y]);
        }
    }

    // Ensure 7 substitutes (pos: 28)
    let cur_subs = pos_arr
        .iter()
        .filter(|p| p.get("pos").and_then(|v| v.as_u64()).unwrap_or(99) == POS_SUB as u64)
        .count();
    let needed_subs = 7usize.saturating_sub(cur_subs);
    for _ in 0..needed_subs {
        if let Some(p) = available_players.pop() {
            let pid = iid(p);
            let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
            pos_arr.push(json!({
                "id": pid,
                "spid": spid,
                "pos": POS_SUB
            }));
        }
    }

    // Validate and fill captain and kickers from starters
    let xi: Vec<String> = pos_arr
        .iter()
        .filter(|p| p.get("pos").and_then(|v| v.as_u64()).unwrap_or(99) < POS_SUB as u64)
        .filter_map(|p| p.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    let cur_cap = sq_obj.get("captain").and_then(|v| v.as_str()).unwrap_or("");
    if (!xi.contains(&cur_cap.to_string()) || cur_cap.is_empty()) && !xi.is_empty() {
        let new_cap = xi.get(8).or_else(|| xi.first()).unwrap();
        sq_obj.insert("captain".to_string(), json!(new_cap));
    }

    let mut kickers = sq_obj
        .get("kickers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|k| k.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    while kickers.len() < 5 {
        kickers.push(String::new());
    }
    kickers.truncate(5);

    for (i, k) in kickers.iter_mut().enumerate() {
        if !xi.contains(k) && !xi.is_empty() {
            let fallback_idx = xi.len().saturating_sub(5);
            let fallback_xi = &xi[fallback_idx..];
            *k = fallback_xi[i % fallback_xi.len()].clone();
        }
    }
    sq_obj.insert("kickers".to_string(), json!(kickers));
    sq_obj.insert("positions".to_string(), Value::Array(pos_arr));
}

pub fn get_default_team(teamid: u64) -> Value {
    let candidate_paths = [
        format!("responses/static_info_authentic_team_all_{}.json", teamid),
        format!("{}/responses/static_info_authentic_team_all_{}.json", env!("CARGO_MANIFEST_DIR"), teamid),
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
            "formations": [],
            "staffs": [],
            "staffroles": [],
            "staffroleidx": 0,
            "maxstaff": 100,
            "staffsmc": {}
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
            obj.insert("levelinfo".to_string(), json!({"level": 1, "exp": 0, "to": 100, "need": 100}));
            let price = card_price(spid, 0);
            obj.insert("price".to_string(), json!(price));
        }
    }

    let mut squad = team.get("squad").cloned().unwrap_or(json!({}));

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
            sq_obj.insert("captain".to_string(), json!(cap.to_string()));
        }
        if let Some(kickers) = sq_obj.get_mut("kickers").and_then(|v| v.as_array_mut()) {
            for k in kickers {
                if let Some(kid) = k.as_u64() {
                    *k = json!(kid.to_string());
                }
            }
        }
    }

    fill_squad(&mut squad, &players);

    team["players"] = Value::Array(players);
    let cap_val = squad.get("captain").cloned().unwrap_or(json!(""));
    let kick_val = squad.get("kickers").cloned().unwrap_or(json!([]));
    team["squad"] = squad;
    team["captain"] = cap_val;
    team["kickers"] = kick_val;
    team["presetidx"] = json!(0);
    team["staffs"] = json!([]);
    team["staffroles"] = default_staffroles();
    team["staffroleidx"] = json!(1);
    team["maxstaff"] = json!(100);
    team["staffsmc"] = json!(0);
    team["formations"] = get_formations();
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
                "messages": [],
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
            if let Some(teams) = v.get_mut("teams").and_then(|t| t.as_object_mut()) {
                for (_k, team_val) in teams.iter_mut() {
                    let players_clone = team_val.get("players").and_then(|p| p.as_array()).cloned().unwrap_or_default();
                    let (cap_val, kick_val) = if let Some(sq) = team_val.get_mut("squad") {
                        fill_squad(sq, &players_clone);
                        (
                            sq.get("captain").cloned().unwrap_or(json!("")),
                            sq.get("kickers").cloned().unwrap_or(json!([])),
                        )
                    } else {
                        (json!(""), json!([]))
                    };
                    team_val["captain"] = cap_val;
                    team_val["kickers"] = kick_val;
                    team_val["formations"] = get_formations();
                    team_val["presetidx"] = json!(0);

                    if let Some(players) = team_val.get_mut("players").and_then(|p| p.as_array_mut()) {
                        for p in players {
                            let pinfo = player_levelinfo(p);
                            if let Some(obj) = p.as_object_mut() {
                                obj.insert("levelinfo".to_string(), pinfo);
                            }
                        }
                    }
                    team_val["staffs"] = json!([]);
                    if team_val.get("staffroles").is_none()
                        || team_val["staffroles"].as_array().map(|a| a.is_empty()).unwrap_or(true)
                    {
                        team_val["staffroles"] = default_staffroles();
                    }
                    team_val["staffroleidx"] = json!(1);
                    team_val["maxstaff"] = json!(100);
                    team_val["staffsmc"] = json!(0);
                }
            }
            if let Some(coaches) = v.get_mut("coaches").and_then(|c| c.as_array_mut()) {
                for c in coaches {
                    let cinfo = coach_levelinfo(c);
                    if let Some(obj) = c.as_object_mut() {
                        obj.insert("levelinfo".to_string(), cinfo);
                    }
                }
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
            },
            "presetidx": 0,
            "formations": [],
            "staffs": [],
            "staffroles": [],
            "staffroleidx": 0,
            "maxstaff": 100,
            "staffsmc": {}
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
    crate::http::pricing::sig3(x as f64).max(0) as u64
}

pub fn price_formula(ovr: u32) -> u64 {
    crate::http::pricing::price_formula(ovr).max(0) as u64
}

pub fn stat_price(ovr: u32) -> u64 {
    crate::http::pricing::stat_price(ovr, None).max(0) as u64
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
    let ovr = card_ovr(spid);
    crate::http::pricing::card_price(ovr, grade as u32, None).max(0) as u64
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

pub fn is_starter(team: &Value, p: &Value) -> bool {
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
            let pos = s.get("pos").and_then(|v| v.as_u64()).unwrap_or(99);
            if pid == id_str && pos < POS_SUB as u64 {
                return true;
            }
        }
    }
    false
}

pub fn add_player(team: &mut Value, spid: u64, grade: u32) -> Value {
    let id = new_iid();
    let first_team_count = team
        .get("players")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|p| !p.get("team2").and_then(|v| v.as_bool()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);
    let price = card_price(spid, grade as usize);
    let mut player = json!({
        "_id": id,
        "id": id,
        "spid": spid,
        "expandme": "player",
        "grade": grade,
        "price": price,
        "jersey": 99,
        "jerseyname": "",
        "life": 100,
        "levelinfo": {"level": 1, "exp": 0, "to": 100}
    });
    if first_team_count >= 36 {
        player["team2"] = json!(true);
    }
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

pub fn add_es<T: Into<i64>>(st: &mut Value, amount: T) {
    let coach = my_coach_mut(st);
    let cur = coach.get("es").and_then(|v| v.as_i64()).unwrap_or(0);
    coach["es"] = json!((cur + amount.into()).max(0));
}

pub fn inventory_add(st: &mut Value, itemid: u64, count: u64) {
    give_pack(st, itemid, count);
}

pub fn get_es(st: &Value) -> i64 {
    my_coach(st).get("es").and_then(|v| v.as_i64()).unwrap_or(0)
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
    let iid = new_iid();
    let entry = json!({
        "_id": iid,
        "id": iid,
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

pub const LR_TICKET: u64 = 801;

pub fn lr_ticket_item() -> Value {
    json!({
        "_id": "lockerroomticket",
        "id": "lockerroomticket",
        "itemid": LR_TICKET,
        "remains": 1,
        "count": 1,
        "expireable": true,
        "expired": false,
        "createat": 1483228800000i64,
        "expireat": VIP_UNTIL,
        "staticdata": {
            "itemid": LR_TICKET,
            "productid": LR_TICKET,
            "name": "Tiket locker room",
            "category": "LOCKERROOM",
            "type": "LOCKERROOM",
            "unit": "COUNT",
            "itemicon": LR_TICKET
        }
    })
}

pub fn item_def(itemid: u64) -> Value {
    crate::http::shop::item_def(itemid)
}

pub fn coach_items(st: &Value, slot: usize, where_key: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let coach = if let Some(coaches) = st.get("coaches").and_then(|v| v.as_array()) {
        coaches
            .iter()
            .find(|c| c.get("slot").and_then(|v| v.as_u64()) == Some(slot as u64))
            .unwrap_or_else(|| my_coach(st))
    } else {
        my_coach(st)
    };

    if let Some(items) = coach.get(where_key).and_then(|v| v.as_array()) {
        for it in items {
            let iid_str = it.get("_id").and_then(|v| v.as_str()).unwrap_or("");
            let itemid = it.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0);
            let remains = it.get("remains").and_then(|v| v.as_u64()).unwrap_or(0);
            if itemid > 0 && remains > 0 {
                out.push(json!({
                    "_id": iid_str,
                    "id": iid_str,
                    "itemid": itemid,
                    "remains": remains,
                    "count": remains,
                    "expireable": true,
                    "expired": false,
                    "staticdata": item_def(itemid),
                }));
            }
        }
    }
    if where_key == "items" {
        out.push(lr_ticket_item());
    }
    out
}

pub fn bag_list(st: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    let coach = my_coach(st);
    if let Some(bag) = coach.get("bag").and_then(|v| v.as_array()) {
        for e in bag {
            let iid_str = e
                .get("_id")
                .or_else(|| e.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title = e.get("title").and_then(|v| v.as_str()).unwrap_or("Quà");
            let msg = e.get("msg").and_then(|v| v.as_str()).unwrap_or("Quà từ server.");
            let createdat = e
                .get("createdat")
                .and_then(|v| v.as_i64())
                .unwrap_or_else(|| Utc::now().timestamp());
            let date = w3c_time(createdat);
            let items = e.get("items").cloned().unwrap_or_else(|| json!([]));
            let players = e.get("players").cloned().unwrap_or_else(|| json!([]));

            let mut bundleicon = EP_ITEM;
            if let Some(icon) = e.get("bundleicon").and_then(|v| v.as_u64()) {
                bundleicon = icon;
            } else if let Some(items_arr) = items.as_array() {
                for it in items_arr {
                    let it_id = it.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0);
                    if it_id != EP_ITEM && it_id != 0 {
                        bundleicon = it_id;
                        break;
                    }
                }
            }

            out.push(json!({
                "_id": iid_str,
                "id": iid_str,
                "type": e.get("type").and_then(|v| v.as_str()).unwrap_or("event"),
                "title": title,
                "msg": msg,
                "date": date,
                "items": items,
                "players": players,
                "bundleicon": bundleicon,
            }));
        }
    }
    out
}

pub fn bag_apply(st: &mut Value, slot: usize, entry: &Value) {
    let entry_id = entry
        .get("_id")
        .or_else(|| entry.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(ep) = entry.get("ep").and_then(|v| v.as_i64()) {
        if ep > 0 {
            add_ep(st, ep);
        }
    }

    if let Some(items) = entry.get("items").and_then(|v| v.as_array()) {
        for it in items {
            let itemid = it.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0);
            let count = it.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
            if itemid == EP_ITEM {
                add_ep(st, count as i64);
            } else if itemid > 0 {
                give_pack(st, itemid, count);
            }
        }
    }

    if let Some(players) = entry.get("players").and_then(|v| v.as_array()) {
        let slot_str = slot.to_string();
        if let Some(teams) = st.get_mut("teams").and_then(|t| t.as_object_mut()) {
            if let Some(team) = teams.get_mut(&slot_str) {
                for p in players {
                    let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                    let grade = p.get("grade").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                    if spid > 0 {
                        add_player(team, spid, grade);
                    }
                }
            }
        }
    }

    let coach = my_coach_mut(st);
    if let Some(bag) = coach.get_mut("bag").and_then(|v| v.as_array_mut()) {
        bag.retain(|e| {
            let eid = e.get("_id").or_else(|| e.get("id")).and_then(|v| v.as_str());
            eid != Some(&entry_id)
        });
    }
}

pub fn inbox_list(st: &Value) -> Vec<Value> {
    let coach = my_coach(st);
    if let Some(msgs) = coach.get("messages").and_then(|v| v.as_array()) {
        msgs.clone()
    } else {
        Vec::new()
    }
}

pub fn inbox_add(st: &mut Value, title: &str, msg: &str, sender: &str) -> Value {
    let now = Utc::now().timestamp();
    let mid = new_iid();
    let entry = json!({
        "_id": mid,
        "id": mid,
        "title": title,
        "message": msg,
        "msg": msg,
        "sender": sender,
        "date": w3c_time(now),
        "senddate": w3c_time(now),
        "createdat": now,
        "type": "notice",
        "isread": false
    });
    let coach = my_coach_mut(st);
    if coach.get("messages").is_none() {
        coach["messages"] = json!([]);
    }
    coach["messages"].as_array_mut().unwrap().push(entry.clone());
    entry
}

pub fn inbox_delete(st: &mut Value, msg_id: Option<&str>) {
    let coach = my_coach_mut(st);
    if let Some(msgs) = coach.get_mut("messages").and_then(|v| v.as_array_mut()) {
        if let Some(id) = msg_id {
            if !id.is_empty() {
                msgs.retain(|m| {
                    let mid = m.get("_id").or_else(|| m.get("id")).and_then(|v| v.as_str());
                    mid != Some(id)
                });
                return;
            }
        }
        msgs.clear();
    }
}

pub fn inbox_mark_read(st: &mut Value, msg_id: Option<&str>) {
    let coach = my_coach_mut(st);
    if let Some(msgs) = coach.get_mut("messages").and_then(|v| v.as_array_mut()) {
        for m in msgs.iter_mut() {
            if let Some(id) = msg_id {
                if !id.is_empty() {
                    let mid = m.get("_id").or_else(|| m.get("id")).and_then(|v| v.as_str());
                    if mid == Some(id) {
                        m["isread"] = json!(true);
                    }
                    continue;
                }
            }
            m["isread"] = json!(true);
        }
    }
}

pub fn stack_move(st: &mut Value, _slot: usize, item_id: &str, src: &str, dst: &str) -> bool {
    let coach = my_coach_mut(st);
    let mut moved: Option<Value> = None;
    if let Some(src_arr) = coach.get_mut(src).and_then(|v| v.as_array_mut()) {
        if let Some(pos) = src_arr.iter().position(|i| i.get("_id").and_then(|v| v.as_str()) == Some(item_id)) {
            moved = Some(src_arr.remove(pos));
        }
    }
    if let Some(it) = moved {
        let itemid = it.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0);
        let remains = it.get("remains").and_then(|v| v.as_u64()).unwrap_or(0);
        if coach.get(dst).is_none() {
            coach[dst] = json!([]);
        }
        let dst_arr = coach.get_mut(dst).and_then(|v| v.as_array_mut()).unwrap();
        if let Some(existing) = dst_arr.iter_mut().find(|i| i.get("itemid").and_then(|v| v.as_u64()) == Some(itemid)) {
            let cur = existing.get("remains").and_then(|v| v.as_u64()).unwrap_or(0);
            existing["remains"] = json!(cur + remains);
        } else {
            dst_arr.push(it);
        }
        true
    } else {
        false
    }
}

pub fn draw_player_for_pack(itemid: u64) -> (u64, u32) {
    let (name, _) = get_pack_info(itemid).unwrap_or(("Player Pack", 100_000));

    let grade: u32 = if name.contains("+8") {
        rand::thread_rng().gen_range(1..=8)
    } else if name.contains("+5") {
        rand::thread_rng().gen_range(1..=5)
    } else if name.contains("+3") {
        rand::thread_rng().gen_range(1..=3)
    } else {
        1
    };

    if Path::new(FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(FIFADB_PATH) {
            let mut conditions: Vec<&str> = Vec::new();
            if name.contains("FW") {
                conditions.push("preferredposition1 BETWEEN 20 AND 27");
            } else if name.contains("MF") {
                conditions.push("preferredposition1 BETWEEN 9 AND 19");
            } else if name.contains("DF") {
                conditions.push("preferredposition1 BETWEEN 1 AND 8");
            } else if name.contains("GK") {
                conditions.push("preferredposition1 = 0");
            }

            if name.contains("Premier League") {
                conditions.push("leagueid = 13");
            } else if name.contains("Bundesliga") {
                conditions.push("leagueid = 19");
            } else if name.contains("LaLiga") {
                conditions.push("leagueid = 53");
            } else if name.contains("Serie A") {
                conditions.push("leagueid = 31");
            } else if name.contains("Ligue 1") {
                conditions.push("leagueid = 16");
            }

            if name.contains("70+") {
                conditions.push("overallrating >= 70");
            } else if name.contains("65+") {
                conditions.push("overallrating >= 65");
            } else if name.contains("60+") {
                conditions.push("overallrating >= 60");
            }

            if name.contains("World Best") {
                conditions.push("(spid / 1000000 = 88 OR overallrating >= 80)");
            }

            let where_clause = if conditions.is_empty() {
                "WHERE overallrating >= 65".to_string()
            } else {
                format!("WHERE {}", conditions.join(" AND "))
            };

            let sql = format!("SELECT spid FROM players {} ORDER BY RANDOM() LIMIT 1", where_clause);
            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(spid) = stmt.query_row([], |row| row.get::<_, u64>(0)) {
                    return (spid, grade);
                }
            }
        }
    }

    (88184941, grade)
}

pub fn use_item_or_pack(st: &mut Value, slot: usize, item_id: &str) -> Result<(Value, Option<Value>), String> {
    let (itemid, cur_players_count) = {
        let coach = my_coach(st);
        let items = coach.get("items").and_then(|v| v.as_array()).ok_or("INVALID_ITEM")?;
        let it = items.iter().find(|i| {
            i.get("_id").and_then(|v| v.as_str()) == Some(item_id)
                && i.get("remains").and_then(|v| v.as_u64()).unwrap_or(0) > 0
        }).ok_or("INVALID_ITEM")?;
        let itemid = it.get("itemid").and_then(|v| v.as_u64()).unwrap_or(0);

        let slot_str = slot.to_string();
        let team = st.get("teams").and_then(|t| t.get(&slot_str)).ok_or("INVALID_TEAM")?;
        let count = team.get("players").and_then(|p| p.as_array()).map(|a| a.len()).unwrap_or(0);
        (itemid, count)
    };

    if let Some(it_def) = crate::http::shop::USE_ITEMS.iter().find(|i| i.itemid == itemid) {
        let mut out = json!({});
        if it_def.item_type == "EP_LOTTERY" {
            let ep_gain: i64 = match itemid {
                50019 => 200_000,
                50020 => 500_000,
                50021 => 1_000_000,
                501 => rand::thread_rng().gen_range(10_000..=1_000_000),
                50029 => rand::thread_rng().gen_range(100_000..=20_000_000),
                50028 => rand::thread_rng().gen_range(200_000..=50_000_000),
                908 => rand::thread_rng().gen_range(1_000_000..=10_000_000),
                50840 => rand::thread_rng().gen_range(500_000..=5_000_000),
                50841 => rand::thread_rng().gen_range(5_000_000..=30_000_000),
                119082304 => rand::thread_rng().gen_range(100_000..=100_000_000),
                _ => 100_000,
            };
            add_ep(st, ep_gain);
            out["ep"] = json!(ep_gain);
        } else if it_def.item_type == "ITEM_LOTTERY" {
            let (prize_ep, prize_item) = match itemid {
                50121 => (Some(10_000_000), 8334),
                50122 => (Some(3_000_000), 82214180),
                50123 => (Some(1_000_000), 8223600),
                50124 => (Some(300_000), 28006),
                50125 => (Some(100_000), 82214356),
                200130000 => (Some(200_000), 28009),
                200130001 => (Some(1_000_000), 45007),
                _ => (Some(100_000), 28001),
            };
            if rand::thread_rng().gen_bool(0.5) {
                if let Some(ep_val) = prize_ep {
                    add_ep(st, ep_val);
                    out["ep"] = json!(ep_val);
                }
            } else {
                inventory_add(st, prize_item, 1);
                out["item"] = json!({
                    "itemid": prize_item,
                    "count": 1,
                    "staticdata": item_def(prize_item)
                });
            }
        }

        let coach = my_coach_mut(st);
        if let Some(items) = coach.get_mut("items").and_then(|v| v.as_array_mut()) {
            if let Some(it) = items.iter_mut().find(|i| i.get("_id").and_then(|v| v.as_str()) == Some(item_id)) {
                let r = it.get("remains").and_then(|v| v.as_u64()).unwrap_or(1);
                if r <= 1 {
                    items.retain(|i| i.get("_id").and_then(|v| v.as_str()) != Some(item_id));
                } else {
                    it["remains"] = json!(r - 1);
                }
            }
        }

        return Ok((Value::Null, Some(out)));
    }

    if cur_players_count >= MAX_PLAYERS {
        return Err("Exceed plyer limit".to_string());
    }

    let (spid, grade) = draw_player_for_pack(itemid);

    let slot_str = slot.to_string();
    let p = {
        let teams = st.get_mut("teams").and_then(|t| t.as_object_mut()).ok_or("INVALID_STATE")?;
        let team = teams.get_mut(&slot_str).ok_or("INVALID_TEAM")?;
        add_player(team, spid, grade)
    };

    let price = card_price(spid, grade as usize);
    let player_val = json!({
        "_id": iid(&p),
        "id": iid(&p),
        "spid": spid,
        "grade": grade,
        "price": price
    });

    let coach = my_coach_mut(st);
    if let Some(items) = coach.get_mut("items").and_then(|v| v.as_array_mut()) {
        if let Some(it) = items.iter_mut().find(|i| i.get("_id").and_then(|v| v.as_str()) == Some(item_id)) {
            let r = it.get("remains").and_then(|v| v.as_u64()).unwrap_or(1);
            if r <= 1 {
                items.retain(|i| i.get("_id").and_then(|v| v.as_str()) != Some(item_id));
            } else {
                it["remains"] = json!(r - 1);
            }
        }
    }

    Ok((player_val, None))
}

pub fn format_number(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts = Vec::new();
    while n > 0 {
        let rem = n % 1000;
        n /= 1000;
        if n > 0 {
            parts.push(format!("{:03}", rem));
        } else {
            parts.push(format!("{}", rem));
        }
    }
    parts.reverse();
    parts.join(".")
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
        ("default_B01", "STR_SCOUTER_KR_DEFAULT_B01", 0.0, Some(json!({"require_value": 100_000}))),
        ("igr", "STR_SCOUTER_KR_IGR_A01", 0.0, Some(json!({"require_grade": 2, "require_grade_count": 2}))),
        ("vip", "STR_SCOUTER_KR_VIP_A01", 0.0, Some(json!({"require_season": "88", "require_season_count": 2}))),
        ("event", "STR_SCOUTER_KR_EVENT_A01", 0.0, Some(json!({"require_season": "93,30,27,71,51,61,89,48,31,33", "require_season_count": 2}))),
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

pub fn match_reward(st: &mut Value, stats: &Value) -> Value {
    let me = stats.get("myTeam").and_then(|v| v.as_str()).unwrap_or("home");
    let opp = stats.get("opponentTeam").and_then(|v| v.as_str()).unwrap_or("away");

    let score_me = stats
        .get(me)
        .and_then(|t| t.get("score"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let score_opp = stats
        .get(opp)
        .and_then(|t| t.get("score"))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let won = score_me > score_opp;
    let drew = score_me == score_opp;

    let ep = if won { 2000 } else if drew { 1000 } else { 500 };
    let (cexp, pexp) = if won {
        EXP_WIN
    } else if drew {
        EXP_DRAW
    } else {
        EXP_LOSS
    };

    let reward_fn = |n: u64| {
        json!({
            "total": n,
            "detail": {
                "base": n,
                "extra": 0
            }
        })
    };

    let (cprev, ccur) = gain_coach_exp(my_coach_mut(st), cexp);

    let slot = default_slot(st);
    let mut done = std::collections::HashSet::new();
    let mut rewarded_players = Vec::new();

    if let Some(teams) = st.get_mut("teams").and_then(|t| t.as_object_mut()) {
        if let Some(team) = teams.get_mut(&slot.to_string()) {
            let stats_players = stats
                .get(me)
                .and_then(|t| t.get("players"))
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();

            for s in stats_players {
                let spid = s.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                let pid = spid % 1_000_000;
                let goals = s.get("goals").and_then(|v| v.as_u64()).unwrap_or(0);
                let assists = s.get("assists").and_then(|v| v.as_u64()).unwrap_or(0);

                if let Some(players) = team.get_mut("players").and_then(|p| p.as_array_mut()) {
                    for p in players.iter_mut() {
                        let p_spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                        let p_id = iid(p);
                        if p_spid % 1_000_000 == pid && !done.contains(&p_id) {
                            let n = pexp + 20 * goals + 10 * assists;
                            let (pprev, pcur) = gain_player_exp(p, n);
                            done.insert(p_id.clone());
                            rewarded_players.push(json!({
                                "id": p_id,
                                "spid": p_spid,
                                "prev": pprev,
                                "current": pcur,
                                "reward": reward_fn(n)
                            }));
                            break;
                        }
                    }
                }
            }
        }
    }

    json!({
        "matchRewardBackType": 0,
        "extra": {"selected": {}, "availables": []},
        "selected": [],
        "coach": {
            "reward": reward_fn(cexp),
            "prev": cprev,
            "current": ccur
        },
        "money": {
            "reward": reward_fn(ep)
        },
        "players": rewarded_players,
        "missions": [],
        "exptankinfo": {}
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_and_coach_curves() {
        assert_eq!(player_need(1), 100);
        assert_eq!(player_need(2), 200);
        assert_eq!(player_need(23), 2300);
        assert_eq!(player_need(24), 2400);

        assert_eq!(coach_need(1), 500);
        assert_eq!(coach_need(2), 600);
        assert_eq!(coach_need(100), 10400);

        assert_eq!(coach_cumulative_exp(1), 500);
        assert_eq!(coach_cumulative_exp(2), 1100);
        assert_eq!(coach_cumulative_exp(3), 1800);
    }

    #[test]
    fn test_expplayer_table_generation() {
        let table = get_expplayer();
        assert!(table.is_object());
        let map = table.as_object().unwrap();
        assert_eq!(map.len(), 24);
        for lv in 1..=24 {
            let row = &map[&lv.to_string()];
            assert_eq!(row["requiredplayerexp"], (100 * lv) as u64);
        }
    }

    #[test]
    fn test_expcoach_table_generation() {
        let table = get_expcoach();
        assert!(table.is_object());
        let map = table.as_object().unwrap();
        assert_eq!(map.len(), 100);
        assert_eq!(map["1"]["totalexp"], 500);
        assert_eq!(map["2"]["totalexp"], 1100);
        assert_eq!(map["3"]["totalexp"], 1800);
        assert_eq!(map["100"]["totalexp"], 545000);
    }

    #[test]
    fn test_player_levelinfo_structure() {
        let p1 = json!({});
        let info1 = player_levelinfo(&p1);
        assert_eq!(info1["level"], 1);
        assert_eq!(info1["exp"], 0);
        assert_eq!(info1["to"], 100);

        let p2 = json!({"levelinfo": {"level": 5, "exp": 120, "need": 500}});
        let info2 = player_levelinfo(&p2);
        assert_eq!(info2["level"], 5);
        assert_eq!(info2["exp"], 120);
        assert_eq!(info2["to"], 500);

        let p3 = json!({"levelinfo": {"level": 23, "exp": 999}});
        let info3 = player_levelinfo(&p3);
        assert_eq!(info3["level"], 23);
        assert_eq!(info3["exp"], 0);
        assert_eq!(info3["to"], 2300);
    }

    #[test]
    fn test_gain_player_exp_leveling() {
        let mut player = json!({
            "spid": 100100,
            "levelinfo": {"level": 1, "exp": 0, "to": 100}
        });

        // 1. Partial exp within same level
        let (p1, c1) = gain_player_exp(&mut player, 50);
        assert_eq!(p1["level"], 1);
        assert_eq!(p1["exp"], 0);
        assert_eq!(c1["level"], 1);
        assert_eq!(c1["exp"], 50);
        assert_eq!(c1["to"], 100);

        // 2. Exact level up
        let (p2, c2) = gain_player_exp(&mut player, 50);
        assert_eq!(p2["level"], 1);
        assert_eq!(p2["exp"], 50);
        assert_eq!(c2["level"], 2);
        assert_eq!(c2["exp"], 0);
        assert_eq!(c2["to"], 200);

        // 3. Multi-level jump: lv 2 needs 200, lv 3 needs 300
        // +250 gives lv 3 with 50 exp
        let (p3, c3) = gain_player_exp(&mut player, 250);
        assert_eq!(p3["level"], 2);
        assert_eq!(p3["exp"], 0);
        assert_eq!(c3["level"], 3);
        assert_eq!(c3["exp"], 50);
        assert_eq!(c3["to"], 300);

        // 4. Level cap at PLAYER_MAX_LEVEL (23)
        let (_p4, c4) = gain_player_exp(&mut player, 500_000);
        assert_eq!(c4["level"], 23);
        assert_eq!(c4["exp"], 0);
        assert_eq!(c4["to"], 2300);

        // 5. Overflow exp while at max level stays 0 exp, lv 23
        let (_p5, c5) = gain_player_exp(&mut player, 10_000);
        assert_eq!(c5["level"], 23);
        assert_eq!(c5["exp"], 0);
        assert_eq!(c5["to"], 2300);
    }

    #[test]
    fn test_gain_coach_exp_leveling() {
        let mut coach = json!({
            "levelinfo": {"level": 1, "exp": 0, "to": 500}
        });

        let (p1, c1) = gain_coach_exp(&mut coach, 300);
        assert_eq!(p1["level"], 1);
        assert_eq!(p1["exp"], 0);
        assert_eq!(c1["level"], 1);
        assert_eq!(c1["exp"], 300);
        assert_eq!(c1["to"], 500);

        // Level up to 2: 300 + 300 = 600 -> lv 2, exp 100, to 600
        let (_p2, c2) = gain_coach_exp(&mut coach, 300);
        assert_eq!(c2["level"], 2);
        assert_eq!(c2["exp"], 100);
        assert_eq!(c2["to"], 600);
    }
}
