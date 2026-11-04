use crate::state;
use rand::seq::SliceRandom;
use rand::Rng;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

pub const NO_LIMIT: u64 = 9_000_000_000_000_000;
pub const LEGEND_SEASONS: [u32; 10] = [93, 30, 27, 71, 51, 61, 89, 48, 31, 33];
pub const QUEST_BONUS: f64 = 0.20;

pub const TRADE2_LEVELS: [u64; 15] = [
    0,
    1_000_000,
    3_000_000,
    10_000_000,
    30_000_000,
    100_000_000,
    300_000_000,
    1_000_000_000,
    2_000_000_000,
    4_000_000_000,
    8_000_000_000,
    15_000_000_000,
    30_000_000_000,
    60_000_000_000,
    120_000_000_000,
];

pub const TRADE2_MANAGERS: [&str; 10] = [
    "Budi", "Andi", "Rizky", "Dimas", "Fajar", "Yoga", "Agus", "Hendra", "Bayu", "Eko",
];

#[derive(Debug, Clone)]
pub struct ScoutInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub mult_min: f64,
    pub mult_max: f64,
}

pub const SCOUTS: [ScoutInfo; 5] = [
    ScoutInfo {
        key: "default_B01",
        name: "STR_SCOUTER_KR_DEFAULT_B01",
        mult_min: 0.9,
        mult_max: 1.3,
    },
    ScoutInfo {
        key: "igr",
        name: "STR_SCOUTER_KR_IGR_A01",
        mult_min: 0.9,
        mult_max: 1.4,
    },
    ScoutInfo {
        key: "vip",
        name: "STR_SCOUTER_KR_VIP_A01",
        mult_min: 1.0,
        mult_max: 1.5,
    },
    ScoutInfo {
        key: "event",
        name: "STR_SCOUTER_KR_EVENT_A01",
        mult_min: 1.0,
        mult_max: 1.5,
    },
    ScoutInfo {
        key: "vvip",
        name: "STR_SCOUTER_KR_VVIP_A01",
        mult_min: 1.1,
        mult_max: 1.6,
    },
];

pub fn scout_quest(key: &str) -> Option<Value> {
    match key {
        "default_B01" => Some(json!({ "require_value": 100_000 })),
        "igr" => Some(json!({ "require_grade": 2, "require_grade_count": 2 })),
        "vip" => Some(json!({ "require_season": "88", "require_season_count": 2 })),
        "event" => Some(json!({
            "require_season": "93,30,27,71,51,61,89,48,31,33",
            "require_season_count": 2
        })),
        "vvip" => Some(json!({ "require_value": 1_000_000_000 })),
        _ => None,
    }
}

pub fn normal_offers_count(n: usize) -> usize {
    match n {
        2..=3 => 1,
        4..=6 => 2,
        _ => 3,
    }
}

pub fn ovr_bonus(n: usize) -> i32 {
    match n {
        2 => -10,
        3 => -8,
        4 => -5,
        5 => -3,
        6 => -1,
        7 => 1,
        8 => 3,
        9 => 5,
        10 => 7,
        _ => 0,
    }
}

pub fn trade_grades(gsum: u32) -> HashMap<String, f64> {
    let max_g = (1 + (gsum / 8)).min(10);
    let mut out = HashMap::new();
    for g in 1..=max_g {
        out.insert(g.to_string(), 0.5f64.powi(g as i32 - 1));
    }
    out
}

pub fn pick_weighted_grade(grades: &HashMap<String, f64>, rng: &mut impl Rng) -> u8 {
    let mut items: Vec<(u8, f64)> = grades
        .iter()
        .filter_map(|(k, v)| k.parse::<u8>().ok().map(|g| (g, *v)))
        .collect();
    items.sort_by_key(|(g, _)| *g);

    let total: f64 = items.iter().map(|(_, w)| *w).sum();
    let mut roll = rng.gen::<f64>() * total;
    for (g, w) in items {
        if roll <= w {
            return g;
        }
        roll -= w;
    }
    1
}

pub fn is_legend(spid: u64) -> bool {
    let season = (spid / 1_000_000) as u32;
    LEGEND_SEASONS.contains(&season)
}

pub fn trade2_tp(give: &[Value]) -> u64 {
    let n = give.len();
    let bonus_n = 1.0 + 0.02 * ((n as f64) - 2.0).max(0.0);
    let igr = 1.05;
    let vip = 1.05;

    let base_sum: u64 = give
        .iter()
        .map(|p| {
            let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
            let grade = state::grade_of(p) as usize;
            let mult = if is_legend(spid) && grade >= 1 {
                1.5
            } else {
                1.0
            };
            (state::card_price(spid, grade) as f64 * mult) as u64
        })
        .sum();

    ((base_sum as f64) * bonus_n * igr * vip) as u64
}

pub fn trade2_max_grade(tp: u64) -> u8 {
    let mut level_idx = 0;
    for (i, &lvl_tp) in TRADE2_LEVELS.iter().enumerate() {
        if tp >= lvl_tp {
            level_idx = i;
        } else {
            break;
        }
    }
    (1 + ((9.0 * (level_idx as f64) / 14.0).round() as u8)).min(10)
}

pub fn trade2_table() -> Value {
    let mut tp_map = json!({});
    for (i, &tp) in TRADE2_LEVELS.iter().enumerate() {
        let max_g = 1 + ((9.0 * (i as f64) / 14.0).round() as usize);
        let vals: Vec<usize> = (1..=max_g).collect();
        tp_map[tp.to_string()] = json!({ "value": vals });
    }

    let mut bonus_map = json!({
        "IGR": 1.05,
        "VIP": 1.05
    });
    for n in 2..=10 {
        let bonus = ((1.0 + 0.02 * (n as f64 - 2.0)) * 100.0).round() / 100.0;
        bonus_map[n.to_string()] = json!(bonus);
    }

    let mut normal_map = json!({});
    for n in 2..=10 {
        normal_map[n.to_string()] = json!(normal_offers_count(n));
    }

    json!({
        "bonusplayergroup": { "bonus": 1.5, "grade": 1 },
        "playertrade2_bonus": bonus_map,
        "legendgroup": LEGEND_SEASONS,
        "playertrade2_normal": normal_map,
        "tp": tp_map,
        "trade2_limit": { "max": NO_LIMIT, "max_trade": NO_LIMIT },
        "trade_effect_price": [10_000_000_000u64, 1_000_000_000u64, 100_000_000u64]
    })
}

pub fn trade2_special_players() -> Value {
    let mut map = json!({});
    if Path::new(state::FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(state::FIFADB_PATH) {
            let sql = "SELECT DISTINCT spid FROM players WHERE spid / 1000000 IN (93, 30, 27, 71, 51, 61, 89, 48, 31, 33)";
            if let Ok(mut stmt) = conn.prepare(sql) {
                if let Ok(rows) = stmt.query_map([], |row| row.get::<_, u64>(0)) {
                    for r in rows.flatten() {
                        map[r.to_string()] = json!(1);
                    }
                }
            }
        }
    }
    for &season in &LEGEND_SEASONS {
        let spid = season as u64 * 1_000_000 + 101;
        map[spid.to_string()] = json!(1);
    }
    map
}

pub fn trade2_info() -> Value {
    let mut out = json!({});
    let mut rng = rand::thread_rng();

    for &tp in &TRADE2_LEVELS[1..] {
        let max_g = trade2_max_grade(tp);
        let g = rng.gen_range(1..=max_g);
        let (spid, _) = pick_trade_spid(tp, g as u32, &mut rng);
        let coach = TRADE2_MANAGERS.choose(&mut rng).unwrap();

        out[tp.to_string()] = json!({
            "spid": spid,
            "grade": g,
            "expandme": "player",
            "price": (tp as f64 * 1.1) as u64,
            "coach": coach
        });
    }

    out
}

pub fn trade3_static() -> Value {
    let mut normal = json!({});
    for n in 2..=10 {
        normal[n.to_string()] = json!(normal_offers_count(n));
    }

    json!({
        "playertrade3_limit": { "max_trade": NO_LIMIT, "max": NO_LIMIT },
        "vvip_playertrade3_limit": { "max_trade": NO_LIMIT, "max": NO_LIMIT },
        "playertrade3_normal": normal,
        "playertrade3_effect_price": [10_000_000, 3_000_000, 1_000_000]
    })
}

pub fn playertrade_table() -> Value {
    let mut pgrade = json!({});
    for s in 2..=100 {
        pgrade[s.to_string()] = json!(trade_grades(s));
    }
    json!({
        "playergrade": pgrade
    })
}

pub fn pick_trade_spid(target_price: u64, grade: u32, rng: &mut impl Rng) -> (u64, u32) {
    if Path::new(state::FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(state::FIFADB_PATH) {
            let g = (grade as usize).clamp(1, 10);
            let mult = state::GRADE_PRICE[g].max(1.0);
            let base = (target_price as f64 / mult).max(1000.0);
            let estimated_ovr = ((base.ln() - 2.53) / 0.1113).round() as i32;
            let target_ovr = estimated_ovr.clamp(55, 95);

            let sql = "SELECT spid, teamid FROM players WHERE overallrating BETWEEN ?1 AND ?2 ORDER BY RANDOM() LIMIT 1";
            if let Ok(mut stmt) = conn.prepare(sql) {
                if let Ok(row) = stmt.query_row([target_ovr - 3, target_ovr + 3], |r| {
                    Ok((r.get::<_, u64>(0)?, r.get::<_, u32>(1)?))
                }) {
                    return row;
                }
            }
            if let Ok(mut stmt) = conn.prepare("SELECT spid, teamid FROM players ORDER BY RANDOM() LIMIT 1") {
                if let Ok(row) = stmt.query_row([], |r| {
                    Ok((r.get::<_, u64>(0)?, r.get::<_, u32>(1)?))
                }) {
                    return row;
                }
            }
        }
    }
    (1_000_300 + rng.gen_range(1..1000), 241)
}

pub fn quest_met(quest: Option<&Value>, give: &[Value]) -> bool {
    let q = match quest {
        Some(q) => q,
        None => return false,
    };

    if let Some(req_val) = q.get("require_value").and_then(|v| v.as_u64()) {
        let total_val: u64 = give
            .iter()
            .map(|p| {
                let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                let grade = state::grade_of(p) as usize;
                state::card_price(spid, grade)
            })
            .sum();
        if total_val < req_val {
            return false;
        }
    }

    if let Some(req_grade) = q.get("require_grade").and_then(|v| v.as_u64()) {
        let req_count = q
            .get("require_grade_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let count = give
            .iter()
            .filter(|p| state::grade_of(p) >= req_grade as usize)
            .count();
        if count < req_count {
            return false;
        }
    }

    if let Some(req_season) = q.get("require_season").and_then(|v| v.as_str()) {
        let req_count = q
            .get("require_season_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let seasons: Vec<u64> = req_season
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if !seasons.is_empty() {
            let count = give
                .iter()
                .filter(|p| {
                    let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                    seasons.contains(&(spid / 1_000_000))
                })
                .count();
            if count < req_count {
                return false;
            }
        }
    }

    true
}

pub fn handle_trade3_proposal(body: &[u8], st: &mut Value, slot: usize) -> Value {
    let req: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let scout_key = match req.get("id").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let scout = match SCOUTS.iter().find(|s| s.key == scout_key) {
        Some(s) => s,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let player_entries = match req.get("playerids").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    if player_entries.len() < 2 || player_entries.len() > 10 {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let team = match st.get("teams").and_then(|t| t.get(&slot.to_string())) {
        Some(t) => t.clone(),
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let team_players = match team.get("players").and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let mut give = Vec::new();
    for entry in player_entries {
        let (pid, client_price) = if let Some(s) = entry.as_str() {
            (s, 0u64)
        } else if let Some(obj) = entry.as_object() {
            let pid = obj.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let cp = obj.get("price").and_then(|v| v.as_u64()).unwrap_or(0);
            (pid, cp)
        } else {
            return json!({ "result": false, "reason": "invalid playerid" });
        };

        match team_players.iter().find(|p| state::iid(p) == pid) {
            Some(p) => {
                let is_cadangan = p.get("team2").and_then(|v| v.as_bool()).unwrap_or(false);
                if !is_cadangan || state::in_squad(&team, p) {
                    return json!({ "result": false, "reason": "invalid playerid" });
                }

                let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
                let grade = state::grade_of(p) as usize;
                let actual_price = state::card_price(spid, grade);
                if client_price > 0 && client_price != actual_price {
                    return json!({ "result": false, "reason": "TRADE3_NOTICE_PLAYERPRICE03" });
                }

                give.push(p.clone());
            }
            None => return json!({ "result": false, "reason": "invalid playerid" }),
        }
    }

    if give.len() != player_entries.len() {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let total: u64 = give
        .iter()
        .map(|p| {
            let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
            let grade = state::grade_of(p) as usize;
            state::card_price(spid, grade)
        })
        .sum();

    if total >= NO_LIMIT {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let quest = scout_quest(scout.key);
    let is_quest_met = quest_met(quest.as_ref(), &give);
    let mut lo = scout.mult_min;
    let mut hi = scout.mult_max;
    if is_quest_met {
        lo += QUEST_BONUS;
        hi += QUEST_BONUS;
    }

    let n = give.len();
    let normal = normal_offers_count(n);
    let mut props = Vec::new();
    let mut rng = rand::thread_rng();

    for i in 0..(normal + 2) {
        let is_igr = i == normal;
        let is_vip = i == normal + 1;
        let boost = if is_igr {
            Some("igr")
        } else if is_vip {
            Some("vip")
        } else {
            None
        };

        let target = ((total as f64) * rng.gen_range(lo..hi)) as u64;
        let g = rng.gen_range(1..=5);
        let (spid, teamid) = pick_trade_spid(target, g as u32, &mut rng);
        let price = target.max(state::card_price(spid, g as usize));

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": teamid,
            "price": price
        });

        if i == normal - 1 && normal > 1 {
            let ep_topup = state::sig3(((total as f64) * rng.gen_range(0.05..0.15)) as u64).max(1000);
            offer["ep"] = json!(ep_topup);
        }

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    let ids: Vec<Value> = give.iter().map(|p| p["_id"].clone()).collect();
    st["trade"] = json!({
        "players": ids,
        "props": props
    });
    let _ = state::save_state(&st);

    let coach_me_val = crate::http::routes::build_coach_me(st, slot);
    json!({
        "result": true,
        "props": props,
        "resources": [
            { "uri": "/coach/me", "body": coach_me_val }
        ]
    })
}

pub fn handle_trade2_proposal(body: &[u8], st: &mut Value, slot: usize) -> Value {
    let req: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let player_entries = match req.get("players").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    if player_entries.len() < 2 || player_entries.len() > 10 {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let team = match st.get("teams").and_then(|t| t.get(&slot.to_string())) {
        Some(t) => t.clone(),
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let team_players = match team.get("players").and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let mut give = Vec::new();
    for entry in player_entries {
        let pid = if let Some(s) = entry.as_str() {
            s
        } else if let Some(obj) = entry.as_object() {
            obj.get("id").and_then(|v| v.as_str()).unwrap_or("")
        } else {
            return json!({ "result": false, "reason": "invalid playerid" });
        };

        match team_players.iter().find(|p| state::iid(p) == pid) {
            Some(p) => {
                if state::in_squad(&team, p) {
                    return json!({ "result": false, "reason": "invalid playerid" });
                }
                give.push(p.clone());
            }
            None => return json!({ "result": false, "reason": "invalid playerid" }),
        }
    }

    if give.len() != player_entries.len() {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let tp = trade2_tp(&give);
    let max_g = trade2_max_grade(tp);
    let normal = normal_offers_count(give.len());
    let mut props = Vec::new();
    let mut rng = rand::thread_rng();

    for i in 0..(normal + 2) {
        let is_igr = i == normal;
        let is_vip = i == normal + 1;
        let boost = if is_igr {
            Some("igr")
        } else if is_vip {
            Some("vip")
        } else {
            None
        };

        let target_price = ((tp as f64) * rng.gen_range(0.85..1.15)) as u64;
        let g = rng.gen_range(1..=max_g);
        let (spid, teamid) = pick_trade_spid(target_price, g as u32, &mut rng);
        let price = target_price.max(state::card_price(spid, g as usize));

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": teamid,
            "price": price
        });

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    let ids: Vec<Value> = give.iter().map(|p| p["_id"].clone()).collect();
    st["trade"] = json!({
        "players": ids,
        "props": props
    });
    let _ = state::save_state(&st);

    json!({
        "result": true,
        "props": props
    })
}

pub fn handle_trade1_proposal(body: &[u8], st: &mut Value, slot: usize) -> Value {
    let req: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(_) => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let player_entries = match req.get("players").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    if player_entries.len() < 2 || player_entries.len() > 10 {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let team = match st.get("teams").and_then(|t| t.get(&slot.to_string())) {
        Some(t) => t.clone(),
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let team_players = match team.get("players").and_then(|p| p.as_array()) {
        Some(p) => p,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let mut give = Vec::new();
    for entry in player_entries {
        let pid = if let Some(s) = entry.as_str() {
            s
        } else if let Some(obj) = entry.as_object() {
            obj.get("id").and_then(|v| v.as_str()).unwrap_or("")
        } else {
            return json!({ "result": false, "reason": "invalid playerid" });
        };

        match team_players.iter().find(|p| state::iid(p) == pid) {
            Some(p) => {
                if state::in_squad(&team, p) {
                    return json!({ "result": false, "reason": "invalid playerid" });
                }
                give.push(p.clone());
            }
            None => return json!({ "result": false, "reason": "invalid playerid" }),
        }
    }

    if give.len() != player_entries.len() {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let n = give.len();
    let total_price: u64 = give
        .iter()
        .map(|p| {
            let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
            let grade = state::grade_of(p) as usize;
            state::card_price(spid, grade)
        })
        .sum();
    let avg_ovr: f64 = give
        .iter()
        .map(|p| state::card_ovr(p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0)) as i32)
        .sum::<i32>() as f64
        / (n as f64);
    let target_ovr = ((avg_ovr + (ovr_bonus(n) as f64)).round() as u32).clamp(50, 100);

    let gsum: u32 = give.iter().map(|p| state::grade_of(p) as u32).sum();
    let grade_weights = trade_grades(gsum);

    let normal = normal_offers_count(n);
    let mut props = Vec::new();
    let mut rng = rand::thread_rng();

    for i in 0..(normal + 2) {
        let is_igr = i == normal;
        let is_vip = i == normal + 1;
        let boost = if is_igr {
            Some("igr")
        } else if is_vip {
            Some("vip")
        } else {
            None
        };

        if i == 1 && boost.is_none() {
            let factor = rng.gen_range(0.9..1.15);
            let ep = state::sig3((total_price as f64 * factor) as u64);
            props.push(json!({
                "teamid": 241,
                "ep": ep
            }));
            continue;
        }

        let g = pick_weighted_grade(&grade_weights, &mut rng);
        let (mut spid, teamid) = pick_trade_spid(total_price, g as u32, &mut rng);
        if spid == 0 {
            spid = 1_000_000 + target_ovr as u64 * 100 + (i as u64);
        }
        let price = state::card_price(spid, g as usize);

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": teamid,
            "price": price
        });

        if i == 2 && boost.is_none() {
            let ep_topup = state::sig3((total_price as f64 * rng.gen_range(0.05..0.20)) as u64);
            offer["ep"] = json!(ep_topup);
        }

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    let ids: Vec<Value> = give.iter().map(|p| p["_id"].clone()).collect();
    st["trade"] = json!({
        "players": ids,
        "props": props
    });
    let _ = state::save_state(&st);

    json!({
        "result": true,
        "props": props
    })
}

pub fn handle_trade_contract(body: &[u8], st: &mut Value, slot: usize) -> Value {
    let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
    let sel = req
        .get("select")
        .or_else(|| req.get("selectnum"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(usize::MAX);

    let tr = match st.get("trade").cloned() {
        Some(t) => t,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let props = match tr.get("props").and_then(|v| v.as_array()) {
        Some(p) => p.clone(),
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let ids = match tr.get("players").and_then(|v| v.as_array()) {
        Some(a) => a.clone(),
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    if sel >= props.len() {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    let offer = props[sel].clone();

    let team = match st.get_mut("teams").and_then(|t| t.get_mut(&slot.to_string())) {
        Some(t) => t,
        None => return json!({ "result": false, "reason": "invalid playerid" }),
    };

    let given_ids: Vec<String> = ids
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();

    let existing_pids: Vec<String> = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|arr| arr.iter().map(state::iid).map(|s| s.to_string()).collect())
        .unwrap_or_default();

    if !given_ids.iter().all(|id| existing_pids.contains(id)) {
        return json!({ "result": false, "reason": "invalid playerid" });
    }

    for pid in &given_ids {
        state::remove_player(team, pid);
    }

    if let Some(spid) = offer.get("spid").and_then(|v| v.as_u64()) {
        let g = offer.get("grade").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
        state::add_player(team, spid, g);
    }

    if let Some(ep) = offer.get("ep").and_then(|v| v.as_u64()) {
        state::add_ep(st, ep as i64);
    }

    if let Some(obj) = st.as_object_mut() {
        obj.remove("trade");
    }

    let _ = state::save_state(&st);

    let coach_me_val = crate::http::routes::build_coach_me(st, slot);
    json!({
        "result": true,
        "resources": [
            { "uri": "/coach/me", "body": coach_me_val }
        ]
    })
}

pub fn handle_player_move(
    pid: &str,
    where_to: &str,
    st: &mut Value,
    slot: usize,
) -> Value {
    let team = match st.get_mut("teams").and_then(|t| t.get_mut(&slot.to_string())) {
        Some(t) => t,
        None => return json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" }),
    };

    let is_in_squad = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|players| {
            players
                .iter()
                .any(|p| state::iid(p) == pid && state::in_squad(team, p))
        })
        .unwrap_or(false);

    let first_team_count = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|players| {
            players
                .iter()
                .filter(|p| !p.get("team2").and_then(|v| v.as_bool()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    let player = match team
        .get_mut("players")
        .and_then(|p| p.as_array_mut())
        .and_then(|players| players.iter_mut().find(|p| state::iid(p) == pid))
    {
        Some(p) => p,
        None => return json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" }),
    };

    if where_to == "secondteam" {
        if is_in_squad {
            return json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" });
        }
        player["team2"] = json!(true);
    } else if where_to == "firstteam" {
        let is_team2 = player.get("team2").and_then(|v| v.as_bool()).unwrap_or(false);
        if is_team2 && first_team_count >= 36 {
            return json!({
                "result": false,
                "reason": "FIRSTTEAM_REGISTRATION_FAIL_POPUP_TITLE"
            });
        }
        if let Some(obj) = player.as_object_mut() {
            obj.remove("team2");
        }
    }

    let _ = state::save_state(&st);

    let coach_me_val = crate::http::routes::build_coach_me(st, slot);
    json!({
        "result": true,
        "resources": [
            { "uri": "/coach/me", "body": coach_me_val }
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade2_tp_levels() {
        assert_eq!(TRADE2_LEVELS.len(), 15);
        assert_eq!(trade2_max_grade(0), 1);
        assert_eq!(trade2_max_grade(120_000_000_000), 10);
    }

    #[test]
    fn test_scout_quests() {
        let players = vec![
            json!({
                "_id": "p1",
                "spid": 93_000_100,
                "grade": 2,
                "team2": true,
            }),
            json!({
                "_id": "p2",
                "spid": 93_000_101,
                "grade": 2,
                "team2": true,
            }),
        ];

        let quest = json!({
            "require_grade": 2,
            "require_grade_count": 2
        });
        assert!(quest_met(Some(&quest), &players));

        let legend_quest = json!({
            "require_season": "93,30",
            "require_season_count": 2
        });
        assert!(quest_met(Some(&legend_quest), &players));
    }

    #[test]
    fn test_trade1_formulas() {
        assert_eq!(normal_offers_count(2), 1);
        assert_eq!(normal_offers_count(3), 1);
        assert_eq!(normal_offers_count(4), 2);
        assert_eq!(normal_offers_count(7), 3);

        assert_eq!(ovr_bonus(2), -10);
        assert_eq!(ovr_bonus(4), -5);
        assert_eq!(ovr_bonus(7), 1);
        assert_eq!(ovr_bonus(10), 7);

        let g = trade_grades(2);
        assert_eq!(g.len(), 1);
        let g40 = trade_grades(40);
        assert_eq!(g40.len(), 6);
    }
}
