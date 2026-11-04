use super::common::{GameState, SharedState};
use super::league::{league_round, sim_score};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CupMeta {
    pub metaid: u32,
    pub _id: String,
    pub name: String,
    pub r#type: String,
    pub teamselect: String,
    pub visible: bool,
    pub difficulty: u32,
    pub quality: u32,
    pub matchcount: u32,
    pub teamcount_desc: u32,
    pub proviso_desc: String,
    pub trophy: String,
    pub skin: String,
    pub bestrewards: Vec<Vec<RewardItem>>,
    pub startat: String,
    pub endat: String,
    pub joinable: Joinable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardItem {
    pub itemid: u32,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Joinable {
    pub fee: u32,
    pub igr: bool,
    pub vip: u32,
    pub allteams: bool,
    pub dayofweek: Vec<u8>,
}

pub fn cup_meta(
    metaid: u32,
    name: &str,
    trophy: &str,
    quality: u32,
    matchcount: u32,
    teams: u32,
    reward: u64,
) -> CupMeta {
    CupMeta {
        metaid,
        _id: metaid.to_string(),
        name: name.to_string(),
        r#type: "pve".to_string(),
        teamselect: "user".to_string(),
        visible: true,
        difficulty: 0,
        quality,
        matchcount,
        teamcount_desc: teams,
        proviso_desc: "PVECUP_INFO_OPTION_NO".to_string(),
        trophy: trophy.to_string(),
        skin: metaid.to_string(),
        bestrewards: vec![vec![RewardItem {
            itemid: 1,
            count: reward,
        }]],
        startat: "2020-01-01T00:00:00.000Z".to_string(),
        endat: "2030-12-31T00:00:00.000Z".to_string(),
        joinable: Joinable {
            fee: 0,
            igr: false,
            vip: 0,
            allteams: true,
            dayofweek: vec![1, 1, 1, 1, 1, 1, 1],
        },
    }
}

pub fn get_cup_metas() -> Vec<CupMeta> {
    vec![
        cup_meta(1, "European Champions Cup", "66", 4, 6, 16, 60000),
        cup_meta(2, "South American Cup", "300000", 3, 6, 16, 40000),
    ]
}

pub fn cup_pool(metaid: u32, mine: u32) -> Vec<u32> {
    let pool1 = vec![
        241, 243, 10, 1, 2, 5, 9, 11, 12, 13, 14, 15, 18, 19, 21, 22, 23, 24, 25, 26,
    ];
    let pool2 = vec![
        1001, 1002, 1003, 1004, 1005, 1006, 1007, 1008, 1009, 1010, 1011, 1012, 1013, 1014, 1015,
        1016, 1017, 1018, 1019, 1020,
    ];

    let source = if metaid == 1 { pool1 } else { pool2 };
    source.into_iter().filter(|&id| id != mine).take(15).collect()
}

pub fn group_table(group: &Value) -> Vec<usize> {
    let teams = group.get("teams").and_then(|t| t.as_array());
    let team_count = teams.map(|t| t.len()).unwrap_or(4);
    let mut table: Vec<(usize, i64, i64, i64)> = (0..team_count).map(|i| (i, 0, 0, 0)).collect();

    if let Some(rounds) = group.get("rounds").and_then(|r| r.as_array()) {
        for rnd in rounds {
            if let Some(pairs) = rnd.get("pairs").and_then(|p| p.as_array()) {
                for pair in pairs {
                    if let Some(leg) = pair.get("legs").and_then(|l| l.get(0)) {
                        if let Some(score) = leg.get("score") {
                            let gh = score.get("home").and_then(|v| v.as_i64()).unwrap_or(0);
                            let ga = score.get("away").and_then(|v| v.as_i64()).unwrap_or(0);
                            let h_idx = leg.get("homeidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                            let a_idx = leg.get("awayidx").and_then(|v| v.as_u64()).map(|x| x as usize);

                            if let (Some(hi), Some(ai)) = (h_idx, a_idx) {
                                if hi < team_count && ai < team_count {
                                    if gh > ga {
                                        table[hi].1 += 3;
                                    } else if gh == ga {
                                        table[hi].1 += 1;
                                        table[ai].1 += 1;
                                    } else {
                                        table[ai].1 += 3;
                                    }
                                    table[hi].2 += gh - ga;
                                    table[ai].2 += ga - gh;
                                    table[hi].3 += gh;
                                    table[ai].3 += ga;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by -points, -gd, -goals
    table.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| b.3.cmp(&a.3))
    });

    table.into_iter().map(|(idx, _, _, _)| idx).collect()
}

pub fn cup_refresh_stats(cup: &mut Value) {
    let mut wdl: HashMap<u32, (u32, u32, u32)> = HashMap::new();

    if let Some(phases) = cup.get("phases").and_then(|p| p.as_array()) {
        for phase in phases {
            if let Some(groups) = phase.get("groups").and_then(|g| g.as_array()) {
                for g in groups {
                    let g_teams = g.get("teams").and_then(|t| t.as_array());
                    if let Some(rounds) = g.get("rounds").and_then(|r| r.as_array()) {
                        for rnd in rounds {
                            if let Some(pairs) = rnd.get("pairs").and_then(|p| p.as_array()) {
                                for pair in pairs {
                                    if let Some(leg) = pair.get("legs").and_then(|l| l.get(0)) {
                                        if let Some(score) = leg.get("score") {
                                            let gh = score.get("home").and_then(|v| v.as_u64()).unwrap_or(0);
                                            let ga = score.get("away").and_then(|v| v.as_u64()).unwrap_or(0);
                                            let hi = leg.get("homeidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                                            let ai = leg.get("awayidx").and_then(|v| v.as_u64()).map(|x| x as usize);

                                            if let (Some(h_idx), Some(a_idx)) = (hi, ai) {
                                                if let Some(teams) = g_teams {
                                                    if let (Some(ht), Some(at)) = (teams.get(h_idx), teams.get(a_idx)) {
                                                        let h_tid = ht.get("teamid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                                        let a_tid = at.get("teamid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

                                                        let entry_h = wdl.entry(h_tid).or_insert((0, 0, 0));
                                                        if gh > ga {
                                                            entry_h.0 += 1;
                                                        } else if gh == ga {
                                                            entry_h.1 += 1;
                                                        } else {
                                                            entry_h.2 += 1;
                                                        }

                                                        let entry_a = wdl.entry(a_tid).or_insert((0, 0, 0));
                                                        if ga > gh {
                                                            entry_a.0 += 1;
                                                        } else if ga == gh {
                                                            entry_a.1 += 1;
                                                        } else {
                                                            entry_a.2 += 1;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(phases) = cup.get_mut("phases").and_then(|p| p.as_array_mut()) {
        for phase in phases {
            let is_rr = phase
                .get("system")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                == "round-robin";

            if let Some(groups) = phase.get_mut("groups").and_then(|g| g.as_array_mut()) {
                for g in groups {
                    let order = if is_rr {
                        group_table(g)
                    } else {
                        let len = g.get("teams").and_then(|t| t.as_array()).map(|t| t.len()).unwrap_or(0);
                        (0..len).collect()
                    };

                    if let Some(teams) = g.get_mut("teams").and_then(|t| t.as_array_mut()) {
                        for (rank, &idx) in order.iter().enumerate() {
                            if let Some(t) = teams.get_mut(idx) {
                                let tid = t.get("teamid").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                let (w, d, l) = wdl.get(&tid).copied().unwrap_or((0, 0, 0));
                                if let Some(obj) = t.as_object_mut() {
                                    obj.insert("win".into(), json!(w));
                                    obj.insert("draw".into(), json!(d));
                                    obj.insert("lose".into(), json!(l));
                                    obj.insert("point".into(), json!(3 * w + d));
                                    obj.insert("rank".into(), json!(rank + 1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn cup_my_pair<'a>(cup: &'a Value) -> (Option<&'a Value>, Option<&'a Value>) {
    let cur = match cup.get("current") {
        Some(c) => c,
        None => return (None, None),
    };

    let phase_idx = cur.get("phase").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let round_idx = cur.get("round").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let leg_idx = cur.get("leg").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    if let Some(phases) = cup.get("phases").and_then(|p| p.as_array()) {
        if let Some(phase) = phases.get(phase_idx) {
            if let Some(groups) = phase.get("groups").and_then(|g| g.as_array()) {
                for g in groups {
                    if let Some(rounds) = g.get("rounds").and_then(|r| r.as_array()) {
                        if let Some(rnd) = rounds.get(round_idx) {
                            if let Some(pairs) = rnd.get("pairs").and_then(|p| p.as_array()) {
                                for pair in pairs {
                                    if let Some(legs) = pair.get("legs").and_then(|l| l.as_array()) {
                                        if let Some(leg) = legs.get(leg_idx) {
                                            let h_idx = leg.get("homeidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                                            let a_idx = leg.get("awayidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                                            let teams = g.get("teams").and_then(|t| t.as_array());

                                            let has_my_coach = |idx: usize| {
                                                teams
                                                    .and_then(|t| t.get(idx))
                                                    .and_then(|t| t.get("coach"))
                                                    .map(|c| !c.is_null())
                                                    .unwrap_or(false)
                                            };

                                            if h_idx.map(&has_my_coach).unwrap_or(false)
                                                || a_idx.map(&has_my_coach).unwrap_or(false)
                                            {
                                                return (Some(g), Some(leg));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    (None, None)
}

pub fn new_cup(st: &GameState, meta: &CupMeta, difficulty: u32) -> Value {
    let my_team = st.my_teamid();
    let coach_id = st.my_coach().map(|c| c._id.clone()).unwrap_or_else(super::common::new_iid);

    let mut pool = cup_pool(meta.metaid, my_team);
    let mut teams = vec![my_team];
    teams.append(&mut pool);

    let mut rng = rand::thread_rng();
    teams.shuffle(&mut rng);

    let cup_team = |tid: u32| {
        json!({
            "teamid": tid,
            "coach": if tid == my_team { Some(coach_id.clone()) } else { None }
        })
    };

    let mut groups = Vec::new();
    for g in 0..4 {
        let g_teams: Vec<Value> = teams[g * 4..g * 4 + 4].iter().map(|&t| cup_team(t)).collect();
        let mut rounds = Vec::new();
        for r in 0..3 {
            let pairs_indices = league_round(&[0, 1, 2, 3], r);
            let pairs: Vec<Value> = pairs_indices
                .into_iter()
                .map(|(h, a)| {
                    json!({
                        "legs": [{
                            "homeidx": h.unwrap(),
                            "awayidx": a.unwrap()
                        }]
                    })
                })
                .collect();

            rounds.push(json!({
                "shuffle": r == 0,
                "pairs": pairs
            }));
        }

        groups.push(json!({
            "teams": g_teams,
            "rounds": rounds
        }));
    }

    let cupid = format!("{:08x}{:016x}", chrono::Utc::now().timestamp(), rng.gen::<u64>());

    let mut cup = json!({
        "_id": cupid,
        "metaid": meta.metaid,
        "name": meta.name,
        "type": "pve",
        "difficulty": difficulty,
        "current": { "phase": 0, "round": 0, "leg": 0 },
        "phases": [{
            "system": "round-robin",
            "name": "Fase grup",
            "maxrounds": 3,
            "groups": groups
        }],
        "coaches": [{
            "_id": coach_id,
            "state": "playing"
        }],
        "teams": teams.iter().map(|&t| cup_team(t)).collect::<Vec<_>>()
    });

    cup_refresh_stats(&mut cup);
    cup
}

pub fn cup_play_round(
    _st: &mut GameState,
    cup: &mut Value,
    mine: Option<(u32, u32, Option<u32>, Option<u32>)>,
) {
    let mut rng = rand::thread_rng();

    let (phase_idx, round_idx, leg_idx) = {
        let cur = &cup["current"];
        (
            cur["phase"].as_u64().unwrap_or(0) as usize,
            cur["round"].as_u64().unwrap_or(0) as usize,
            cur["leg"].as_u64().unwrap_or(0) as usize,
        )
    };

    let is_ko = cup["phases"][phase_idx]["system"].as_str().unwrap_or("") == "knock-out";

    // Play matches in current round
    if let Some(groups) = cup["phases"][phase_idx]["groups"].as_array_mut() {
        for g in groups {
            let g_teams = g["teams"].as_array().cloned().unwrap_or_default();
            if let Some(pair) = g["rounds"][round_idx]["pairs"].as_array_mut() {
                for p in pair {
                    if let Some(leg) = p["legs"][leg_idx].as_object_mut() {
                        if leg.contains_key("score") || !leg.contains_key("homeidx") {
                            continue;
                        }

                        let hi = leg["homeidx"].as_u64().unwrap() as usize;
                        let ai = leg["awayidx"].as_u64().unwrap() as usize;

                        let h_coach = g_teams.get(hi).and_then(|t| t.get("coach")).map(|c| !c.is_null()).unwrap_or(false);
                        let a_coach = g_teams.get(ai).and_then(|t| t.get("coach")).map(|c| !c.is_null()).unwrap_or(false);
                        let is_mine = h_coach || a_coach;

                        let (gh, ga) = if is_mine && mine.is_some() {
                            let m = mine.unwrap();
                            (m.0, m.1)
                        } else {
                            sim_score(&mut rng, 75.0, 75.0)
                        };

                        leg.insert("score".into(), json!({ "home": gh, "away": ga }));

                        if is_ko && gh == ga {
                            if is_mine && mine.and_then(|m| m.2).is_some() {
                                let m = mine.unwrap();
                                leg.insert(
                                    "shootout".into(),
                                    json!({ "home": m.2.unwrap(), "away": m.3.unwrap() }),
                                );
                            } else {
                                let ph = rng.gen_range(3..=5);
                                let pa = if ph < 5 {
                                    if rng.gen_bool(0.5) { ph + 1 } else { ph - 1 }
                                } else {
                                    ph - 1
                                };
                                leg.insert("shootout".into(), json!({ "home": ph, "away": pa }));
                            }
                        }
                    }
                }
            }
        }
    }

    if !is_ko {
        // Group phase round advancement
        let max_rounds = cup["phases"][phase_idx]["groups"][0]["rounds"].as_array().map(|r| r.len()).unwrap_or(3);
        if round_idx + 1 < max_rounds {
            cup["current"]["round"] = json!(round_idx + 1);
            return;
        }

        // Group phase complete -> form knockout phase (Quarterfinals)
        // Group tables: A(0), B(1), C(2), D(3)
        // QF pairings: A1 vs B2, B1 vs A2, C1 vs D2, D1 vs C2
        let mut tables = Vec::new();
        if let Some(groups) = cup["phases"][phase_idx]["groups"].as_array() {
            for g in groups {
                tables.push(group_table(g));
            }
        }

        let mut qual = Vec::new();
        for &(x, y) in &[(0, 1), (2, 3)] {
            if tables.len() > x && tables.len() > y {
                let gx = &cup["phases"][phase_idx]["groups"][x]["teams"];
                let gy = &cup["phases"][phase_idx]["groups"][y]["teams"];
                qual.push(gx[tables[x][0]].clone());
                qual.push(gy[tables[y][1]].clone());
                qual.push(gy[tables[y][0]].clone());
                qual.push(gx[tables[x][1]].clone());
            }
        }

        let qf_pairs: Vec<Value> = (0..8)
            .step_by(2)
            .map(|i| {
                json!({
                    "legs": [{
                        "homeidx": i,
                        "awayidx": i + 1
                    }]
                })
            })
            .collect();

        let sf_pairs: Vec<Value> = (0..2)
            .map(|_| json!({ "legs": [{}] }))
            .collect();

        let f_pairs: Vec<Value> = vec![json!({ "legs": [{}] })];

        let ko_phase = json!({
            "system": "knock-out",
            "name": "Babak gugur",
            "maxrounds": 3,
            "groups": [{
                "teams": qual,
                "rounds": [
                    { "shuffle": true, "pairs": qf_pairs },
                    { "pairs": sf_pairs },
                    { "pairs": f_pairs }
                ]
            }]
        });

        cup["phases"].as_array_mut().unwrap().push(ko_phase);
        cup["current"] = json!({
            "phase": phase_idx + 1,
            "round": 0,
            "leg": 0
        });

        // Check if user coach qualified to knockout
        let my_qualified = qual.iter().any(|t| {
            t.get("coach").map(|c| !c.is_null()).unwrap_or(false)
        });
        if !my_qualified {
            cup["coaches"][0]["state"] = json!("eliminated");
        }
        return;
    }

    // Knockout round advancement
    let (winners, my_idx, max_rounds) = {
        let g = &cup["phases"][phase_idx]["groups"][0];
        let mut winners = Vec::new();
        if let Some(pairs) = g["rounds"][round_idx]["pairs"].as_array() {
            for p in pairs {
                let leg = &p["legs"][0];
                let s = leg.get("shootout").unwrap_or(&leg["score"]);
                let gh = s["home"].as_u64().unwrap_or(0);
                let ga = s["away"].as_u64().unwrap_or(0);
                let winner = if gh > ga {
                    leg["homeidx"].as_u64().unwrap() as usize
                } else {
                    leg["awayidx"].as_u64().unwrap() as usize
                };
                winners.push(winner);
            }
        }

        let my_idx = g["teams"].as_array().and_then(|teams| {
            teams.iter().position(|t| t.get("coach").map(|c| !c.is_null()).unwrap_or(false))
        });
        let max_rounds = g["rounds"].as_array().map(|r| r.len()).unwrap_or(3);
        (winners, my_idx, max_rounds)
    };

    if let Some(idx) = my_idx {
        if cup["coaches"][0]["state"].as_str() == Some("playing") && !winners.contains(&idx) {
            cup["coaches"][0]["state"] = json!("eliminated");
        }
    }
    if round_idx + 1 < max_rounds {
        let next_round = round_idx + 1;
        let next_pairs = cup["phases"][phase_idx]["groups"][0]["rounds"][next_round]["pairs"]
            .as_array_mut()
            .unwrap();

        for (k, pair) in next_pairs.iter_mut().enumerate() {
            pair["legs"][0] = json!({
                "homeidx": winners[2 * k],
                "awayidx": winners[2 * k + 1]
            });
        }
        cup["current"]["round"] = json!(next_round);
    } else {
        cup["finished"] = json!(true);
        if cup["coaches"][0]["state"].as_str() == Some("playing") {
            let won = my_idx.map(|idx| winners.contains(&idx)).unwrap_or(false);
            cup["coaches"][0]["state"] = json!(if won { "won" } else { "eliminated" });
        }
    }
}

pub fn cup_reward_ep(cup: &Value) -> u64 {
    let state = cup["coaches"][0]["state"].as_str().unwrap_or("playing");
    let metaid = cup.get("metaid").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let prize = if metaid == 1 { 60000u64 } else { 40000u64 };

    if state == "won" {
        return prize;
    }
    if state != "eliminated" {
        return 0;
    }

    let phases = cup.get("phases").and_then(|p| p.as_array());
    let ko = phases.and_then(|p| p.get(1)).and_then(|p| p.get("groups")).and_then(|g| g.get(0));

    if let Some(ko_group) = ko {
        let my_idx = ko_group["teams"].as_array().and_then(|teams| {
            teams.iter().position(|t| t.get("coach").map(|c| !c.is_null()).unwrap_or(false))
        });

        if let Some(idx) = my_idx {
            let mut played = 0;
            if let Some(rounds) = ko_group.get("rounds").and_then(|r| r.as_array()) {
                for r in rounds {
                    if let Some(pairs) = r.get("pairs").and_then(|p| p.as_array()) {
                        for p in pairs {
                            let leg = &p["legs"][0];
                            let hi = leg.get("homeidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                            let ai = leg.get("awayidx").and_then(|v| v.as_u64()).map(|x| x as usize);
                            if (hi == Some(idx) || ai == Some(idx)) && leg.get("score").is_some() {
                                played += 1;
                            }
                        }
                    }
                }
            }

            match played {
                1 => prize * 20 / 100, // QF exit
                2 => prize * 35 / 100, // SF exit
                3 => prize * 50 / 100, // Final exit
                _ => prize * 20 / 100,
            }
        } else {
            prize / 10 // Group exit: 10%
        }
    } else {
        prize / 10 // Group exit: 10%
    }
}

pub fn cup_resources(cup: &Value) -> Vec<Value> {
    let cid = cup.get("_id").and_then(|v| v.as_str()).unwrap_or("");
    let uri = format!("/cup/{}", cid);
    vec![json!({
        "resource": uri,
        "uri": uri,
        "body": cup
    })]
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

pub async fn ep_cup_list() -> impl IntoResponse {
    Json(get_cup_metas())
}

pub async fn ep_cup_record(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.read().await;
    let coach_id = st.my_coach().map(|c| c._id.clone()).unwrap_or_default();
    Json(json!({
        "result": true,
        "resources": [{
            "resource": "/cup/record",
            "uri": "/cup/record",
            "body": {
                "_id": coach_id,
                "recordsbymetaid": []
            }
        }]
    }))
}

#[derive(Debug, Deserialize)]
pub struct NewCupReq {
    pub metaid: Option<u32>,
    pub difficulty: Option<u32>,
}

pub async fn ep_cup_new(
    State(state): State<SharedState>,
    Json(req): Json<NewCupReq>,
) -> impl IntoResponse {
    let metaid = req.metaid.unwrap_or(1);
    let metas = get_cup_metas();
    let meta = match metas.into_iter().find(|m| m.metaid == metaid) {
        Some(m) => m,
        None => return Json(json!({ "result": false, "reason": "invalid metaid" })),
    };

    let mut st = state.write().await;
    let cup = new_cup(&st, &meta, req.difficulty.unwrap_or(0));
    let cupid = cup["_id"].as_str().unwrap().to_string();

    let res = cup_resources(&cup);
    st.cups.insert(cupid, cup);

    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_cup_match(
    State(state): State<SharedState>,
    Path(cupid): Path<String>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let cup = match st.cups.get(&cupid) {
        Some(c) => c.clone(),
        None => return Json(json!({ "result": false, "reason": "not exist cupid" })),
    };

    let (g, leg) = cup_my_pair(&cup);
    if leg.is_none() || leg.unwrap().get("score").is_some() {
        return Json(json!({ "result": false, "reason": "already played match" }));
    }

    let g_val = g.unwrap();
    let leg_val = leg.unwrap();

    let hi = leg_val["homeidx"].as_u64().unwrap() as usize;
    let ai = leg_val["awayidx"].as_u64().unwrap() as usize;

    let home = g_val["teams"][hi]["teamid"].as_u64().unwrap() as u32;
    let away = g_val["teams"][ai]["teamid"].as_u64().unwrap() as u32;
    let my_team = st.my_teamid();

    let match_id = format!("{:08x}{:016x}", chrono::Utc::now().timestamp(), 0x11223344u64);
    let is_mine_home = home == my_team;

    let match_side = |tid: u32| {
        json!({
            "ready": false,
            "team": {
                "teamid": tid,
                "kit": {
                    "availables": {
                        "away": { "id": (tid << 5) | 1 },
                        "home": { "id": (tid << 5) }
                    }
                }
            }
        })
    };

    let cur_phase = cup["current"]["phase"].as_u64().unwrap_or(0) as usize;
    let is_ko = cup["phases"][cur_phase]["system"].as_str() == Some("knock-out");

    let mut gs = json!({
        "home": is_mine_home,
        "kit": if is_mine_home { ["home", "away"] } else { ["away", "home"] },
        "difficulty": cup.get("difficulty").and_then(|v| v.as_u64()).unwrap_or(0)
    });

    if is_ko {
        gs["drawMode"] = json!(2); // extra time
    }

    let match_body = json!({
        "id": match_id,
        "type": "Cup_PvE",
        "stage": "created",
        "gamesettings": gs,
        "side": {
            "left": match_side(if is_mine_home { home } else { away }),
            "right": match_side(if is_mine_home { away } else { home })
        }
    });

    st.cup_pending = Some(super::common::CupPending {
        id: match_id.clone(),
        cupid: cupid.clone(),
        home,
        away,
    });
    st.match_data = Some(match_body.clone());

    Json(json!({
        "result": true,
        "resources": [{
            "uri": format!("/match/{}", match_id),
            "body": match_body
        }]
    }))
}

pub async fn ep_cup_next(
    State(state): State<SharedState>,
    Path(cupid): Path<String>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let mut cup = match st.cups.get_mut(&cupid) {
        Some(c) => c.clone(),
        None => return Json(json!({ "result": false, "reason": "not exist cupid" })),
    };

    if !cup.get("finished").and_then(|v| v.as_bool()).unwrap_or(false) {
        // Mine score can be stored in cup["_mine"]
        let mine: Option<(u32, u32, Option<u32>, Option<u32>)> = cup
            .as_object_mut()
            .and_then(|m| m.remove("_mine"))
            .and_then(|v| {
                let arr = v.as_array()?;
                let gh = arr.get(0)?.as_u64()? as u32;
                let ga = arr.get(1)?.as_u64()? as u32;
                let sh = arr.get(2).and_then(|x| x.as_u64()).map(|x| x as u32);
                let sa = arr.get(3).and_then(|x| x.as_u64()).map(|x| x as u32);
                Some((gh, ga, sh, sa))
            });

        cup_play_round(&mut st, &mut cup, mine);
        cup_refresh_stats(&mut cup);
        st.cups.insert(cupid.clone(), cup.clone());
    }

    let res = cup_resources(&cup);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_cup_leave(
    State(state): State<SharedState>,
    Path(cupid): Path<String>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let cup = st.cups.remove(&cupid);
    let ep = cup.as_ref().map(cup_reward_ep).unwrap_or(0);

    if ep > 0 {
        st.add_ep(ep as i64);
    }

    let rewards = if ep > 0 {
        vec![json!({ "itemid": 1, "count": ep })]
    } else {
        vec![]
    };

    Json(json!({
        "result": true,
        "rewards": rewards
    }))
}

pub async fn ep_cup_resume(
    State(state): State<SharedState>,
    Path(cupid): Path<String>,
) -> impl IntoResponse {
    let st = state.read().await;
    let cup = match st.cups.get(&cupid) {
        Some(c) => c,
        None => return Json(json!({ "result": false, "reason": "not exist cupid" })),
    };
    let res = cup_resources(cup);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_cup_get(
    State(state): State<SharedState>,
    Path(cupid): Path<String>,
) -> impl IntoResponse {
    let st = state.read().await;
    let cup = st.cups.get(&cupid).cloned().unwrap_or(json!({}));
    Json(cup)
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/cup/list", get(ep_cup_list))
        .route("/cup/record", get(ep_cup_record))
        .route("/cup/new", post(ep_cup_new))
        .route("/cup/{cupid}/match", post(ep_cup_match))
        .route("/cup/{cupid}/next", put(ep_cup_next))
        .route("/cup/{cupid}/leave", put(ep_cup_leave))
        .route("/cup/{cupid}/resume", get(ep_cup_resume).put(ep_cup_resume))
        .route("/cup/{cupid}", get(ep_cup_get))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cup_meta_structure() {
        let metas = get_cup_metas();
        assert_eq!(metas.len(), 2);
        assert_eq!(metas[0].metaid, 1);
        assert_eq!(metas[0].bestrewards[0][0].count, 60000);
        assert_eq!(metas[1].metaid, 2);
        assert_eq!(metas[1].bestrewards[0][0].count, 40000);
    }

    #[test]
    fn test_cup_reward_calculation() {
        let mut cup = json!({
            "metaid": 1,
            "coaches": [{ "state": "won" }]
        });
        assert_eq!(cup_reward_ep(&cup), 60000);

        cup["coaches"][0]["state"] = json!("playing");
        assert_eq!(cup_reward_ep(&cup), 0);

        cup["coaches"][0]["state"] = json!("eliminated");
        // Group exit: 10%
        assert_eq!(cup_reward_ep(&cup), 6000);
    }

    #[test]
    fn test_group_table_ranking() {
        let group = json!({
            "teams": [
                { "teamid": 1 },
                { "teamid": 2 },
                { "teamid": 3 },
                { "teamid": 4 }
            ],
            "rounds": [
                {
                    "pairs": [
                        { "legs": [{ "homeidx": 0, "awayidx": 1, "score": { "home": 3, "away": 0 } }] },
                        { "legs": [{ "homeidx": 2, "awayidx": 3, "score": { "home": 1, "away": 1 } }] }
                    ]
                }
            ]
        });

        let table = group_table(&group);
        assert_eq!(table[0], 0); // Team 0 has 3 pts, +3 gd
        assert_eq!(table[3], 1); // Team 1 has 0 pts, -3 gd
    }
}
