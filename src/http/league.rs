use super::common::{today_ms, GameState, SharedState};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

pub const LEAGUE_SIZE: usize = 10;
pub const SPONSORS: [(&str, &str); 3] = [("1", "2"), ("2", "3"), ("3", "4")];

pub fn mission_target(mid: &str) -> u32 {
    match mid {
        "2" => 3, // finish in top 3
        "3" => 3, // 3 wins in a row
        "4" => 2, // 2 clean sheets in a row
        _ => 1,
    }
}

pub fn sponsor_mid(sid: &str) -> &'static str {
    match sid {
        "1" => "2",
        "2" => "3",
        "3" => "4",
        _ => "2",
    }
}

/// Circle method round-robin scheduler.
/// Second half swaps home/away. None = bye (odd team count).
pub fn league_round(teams: &[u32], rnd: usize) -> Vec<(Option<u32>, Option<u32>)> {
    let n = teams.len();
    let size = n + (n % 2);
    let per = size - 1;
    let flip = ((rnd / per) % 2) == 1;
    let r = rnd % per;

    let rot = |i: usize| -> Option<u32> {
        let k = if i == 0 {
            0
        } else {
            (i - 1 + r) % per + 1
        };
        if k < n {
            Some(teams[k])
        } else {
            None
        }
    };

    let mut pairs = Vec::with_capacity(size / 2);
    for k in 0..(size / 2) {
        let a = rot(k);
        let b = rot(size - 1 - k);
        let f = if k == 0 && (r % 4 > 1) { !flip } else { flip };
        if f {
            pairs.push((b, a));
        } else {
            pairs.push((a, b));
        }
    }
    pairs
}

pub fn poisson(rng: &mut impl Rng, lam: f64) -> u32 {
    let lim = (-lam).exp();
    let mut k = 0;
    let mut p = 1.0;
    loop {
        p *= rng.gen::<f64>();
        if p <= lim {
            return k;
        }
        k += 1;
    }
}

pub fn sim_score(rng: &mut impl Rng, home_strength: f64, away_strength: f64) -> (u32, u32) {
    let d = (home_strength + 3.0 - away_strength) / 10.0;
    let lam_h = (1.4 + 0.6 * d).max(0.3);
    let lam_a = (1.1 - 0.6 * d).max(0.2);
    (poisson(rng, lam_h), poisson(rng, lam_a))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchRecord {
    #[serde(default = "default_match_type")]
    pub r#type: String,
    #[serde(default = "default_match_type")]
    pub matchtype: String,
    pub round: usize,
    pub home: u32,
    pub away: u32,
    pub goals: Goals,
}

fn default_match_type() -> String {
    "league".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goals {
    pub home: u32,
    pub away: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BeforeStats {
    pub rank: u32,
    pub point: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamStats {
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub goals: u32,
    pub goalsconceded: u32,
    pub awaygoals: u32,
    pub point: u32,
    #[serde(default)]
    pub rank: u32,
    #[serde(default)]
    pub before: BeforeStats,
}

pub fn league_stats(
    teams: &[u32],
    records: &[MatchRecord],
    prev: Option<&HashMap<String, TeamStats>>,
) -> HashMap<String, TeamStats> {
    let mut stats: HashMap<String, TeamStats> = HashMap::new();
    for t in teams {
        stats.insert(t.to_string(), TeamStats::default());
    }

    for m in records {
        let gh = m.goals.home;
        let ga = m.goals.away;

        if let Some(h) = stats.get_mut(&m.home.to_string()) {
            h.goals += gh;
            h.goalsconceded += ga;
            if gh > ga {
                h.wins += 1;
                h.point += 3;
            } else if gh == ga {
                h.draws += 1;
                h.point += 1;
            } else {
                h.losses += 1;
            }
        }

        if let Some(a) = stats.get_mut(&m.away.to_string()) {
            a.goals += ga;
            a.goalsconceded += gh;
            a.awaygoals += ga;
            if ga > gh {
                a.wins += 1;
                a.point += 3;
            } else if ga == gh {
                a.draws += 1;
                a.point += 1;
            } else {
                a.losses += 1;
            }
        }
    }

    // Sort teams by -points, -(goals - conceded), -goals, -awaygoals
    let mut ordered = teams.to_vec();
    ordered.sort_by(|&ta, &tb| {
        let sa = &stats[&ta.to_string()];
        let sb = &stats[&tb.to_string()];

        let diff_a = (sa.goals as i64) - (sa.goalsconceded as i64);
        let diff_b = (sb.goals as i64) - (sb.goalsconceded as i64);

        sb.point
            .cmp(&sa.point)
            .then_with(|| diff_b.cmp(&diff_a))
            .then_with(|| sb.goals.cmp(&sa.goals))
            .then_with(|| sb.awaygoals.cmp(&sa.awaygoals))
    });

    for (i, t) in ordered.iter().enumerate() {
        if let Some(s) = stats.get_mut(&t.to_string()) {
            s.rank = (i + 1) as u32;
        }
    }

    for (k, v) in stats.iter_mut() {
        if let Some(p) = prev.and_then(|pr| pr.get(k)) {
            v.before = BeforeStats {
                rank: p.rank,
                point: p.point,
            };
        } else {
            v.before = BeforeStats {
                rank: v.rank,
                point: v.point,
            };
        }
    }

    stats
}

pub fn mission_progress(
    records: &[MatchRecord],
    my_team: u32,
    mid: &str,
    maxround: usize,
    rank: u32,
) -> (Vec<u32>, bool) {
    let mut mine: Vec<&MatchRecord> = records
        .iter()
        .filter(|m| m.home == my_team || m.away == my_team)
        .collect();
    mine.sort_by_key(|m| m.round);

    let target = mission_target(mid);

    if mid == "2" {
        let done = mine.len() > maxround;
        return (vec![rank], done && rank > 0 && rank <= target);
    }

    let is_win = |m: &MatchRecord| -> bool {
        if m.home == my_team {
            m.goals.home > m.goals.away
        } else {
            m.goals.away > m.goals.home
        }
    };

    let is_clean_sheet = |m: &MatchRecord| -> bool {
        if m.home == my_team {
            m.goals.away == 0
        } else {
            m.goals.home == 0
        }
    };

    let mut best_run = 0;
    let mut run = 0;
    for m in &mine {
        let ok = if mid == "3" {
            is_win(m)
        } else {
            is_clean_sheet(m)
        };
        if ok {
            run += 1;
            best_run = best_run.max(run);
        } else {
            run = 0;
        }
    }

    (vec![best_run], best_run >= target)
}

pub fn default_league_teams(mine: u32) -> Vec<u32> {
    let mut pool = vec![243, 241, 10, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 12];
    pool.retain(|&x| x != mine);
    let mut out = vec![mine];
    out.extend(pool.into_iter().take(LEAGUE_SIZE - 1));
    while out.len() < LEAGUE_SIZE {
        out.push(100 + out.len() as u32);
    }
    out
}

pub fn season_leaders(st: &GameState, my_teamid: u32, _rank: u32, goals: u32) -> Value {
    let starters: Vec<u32> = if let Some(t) = st.my_team() {
        t.squad
            .positions
            .iter()
            .filter(|s| s.pos < 18)
            .map(|s| s.spid)
            .collect()
    } else {
        vec![]
    };

    let fw = starters.first().copied().unwrap_or(100001);
    let mf = starters.get(4).copied().unwrap_or(100005);
    let df = starters.get(8).copied().unwrap_or(100009);
    let gk = starters.last().copied().unwrap_or(100011);

    let entry = |spid: u32, value: f64, ep: u64| -> Value {
        json!({
            "spid": spid,
            "teamid": my_teamid,
            "value": value,
            "reward": { "count": ep }
        })
    };

    let played = (2 * (LEAGUE_SIZE - 1)) as f64;

    json!({
        "FW": [entry(fw, 7.5, 0), entry(fw, 7.2, 0), entry(fw, 7.0, 0)],
        "MF": [entry(mf, 7.4, 0), entry(mf, 7.1, 0), entry(mf, 6.9, 0)],
        "DF": [entry(df, 7.3, 0), entry(df, 7.0, 0), entry(df, 6.8, 0), entry(df, 6.7, 0)],
        "GK": [entry(gk, 7.2, 0)],
        "ratings": [entry(fw, 7.5, 3000)],
        "goals": [entry(fw, (goals / 2).max(1) as f64, 3000)],
        "assists": [entry(mf, (goals / 3).max(1) as f64, 2000)],
        "tackles": [entry(df, played * 3.0, 2000)],
        "passes": [entry(mf, 0.85, 1000)],
        "saves": [entry(gk, played * 2.0, 1000)]
    })
}

pub fn l2_me(st: &GameState) -> Value {
    let teamid = st.my_teamid();
    let lg = st.league2.as_ref();

    let leagueid = lg
        .and_then(|l| l.get("leagueid"))
        .and_then(|v| v.as_u64())
        .unwrap_or(13) as u32;

    let difficultyselect = lg
        .and_then(|l| l.get("difficultyselect"))
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as u32;

    let laststatus = lg
        .and_then(|l| l.get("laststatus"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sponsoroffice = lg
        .and_then(|l| l.get("sponsoroffice"))
        .cloned()
        .unwrap_or_else(|| json!({ "season": { "status": "none" } }));

    let mut me = json!({
        "teamid": teamid,
        "leagueid": leagueid,
        "difficultybest": 24,
        "difficultyselect": difficultyselect,
        "laststatus": laststatus,
        "cupinfo": { "entered": false },
        "leader": { "league": {}, "cup": {} },
        "sponsoroffice": sponsoroffice,
    });

    if let Some(league_obj) = lg.and_then(|l| l.get("league")) {
        let openingday = lg
            .and_then(|l| l.get("openingday"))
            .and_then(|v| v.as_u64())
            .unwrap_or_else(today_ms);

        if let Some(m) = me.as_object_mut() {
            m.insert("openingday".to_string(), json!(openingday));
            m.insert("league".to_string(), league_obj.clone());
        }
    }

    me
}

pub fn l2_resources(st: &GameState) -> Vec<Value> {
    vec![json!({
        "uri": "/league2/me",
        "body": l2_me(st)
    })]
}

pub fn league_play_round(
    st: &mut GameState,
    rnd: usize,
    mine: Option<(u32, u32)>,
) -> Value {
    let my_team = st.my_teamid();
    let mut rng = rand::thread_rng();

    let lg = match st.league2.as_mut() {
        Some(l) => l,
        None => return json!({ "matchtype": "league", "round": rnd, "leaguematches": [] }),
    };

    let league_val = match lg.get_mut("league") {
        Some(l) => l,
        None => return json!({ "matchtype": "league", "round": rnd, "leaguematches": [] }),
    };

    let teams: Vec<u32> = league_val
        .get("teams")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|x| x as u32))
                .collect()
        })
        .unwrap_or_default();

    let mut records: Vec<MatchRecord> = league_val
        .get("matchrecords")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_default();

    // If round already played, return those matches
    if records.iter().any(|m| m.round == rnd) {
        let matches: Vec<&MatchRecord> = records.iter().filter(|m| m.round == rnd).collect();
        return json!({
            "matchtype": "league",
            "round": rnd,
            "leaguematches": matches
        });
    }

    let pairs = league_round(&teams, rnd);
    let mut played = Vec::new();

    for (h_opt, a_opt) in pairs {
        if let (Some(h), Some(a)) = (h_opt, a_opt) {
            let (gh, ga) = if let Some(m) = mine {
                if h == my_team || a == my_team {
                    m
                } else {
                    sim_score(&mut rng, 75.0, 75.0)
                }
            } else {
                sim_score(&mut rng, 75.0, 75.0)
            };

            played.push(MatchRecord {
                r#type: "league".to_string(),
                matchtype: "league".to_string(),
                round: rnd,
                home: h,
                away: a,
                goals: Goals { home: gh, away: ga },
            });
        }
    }

    records.extend(played.clone());

    let prev_stats: Option<HashMap<String, TeamStats>> = league_val
        .get("statistics")
        .and_then(|s| serde_json::from_value(s.clone()).ok());

    let new_stats = league_stats(&teams, &records, prev_stats.as_ref());

    league_val["matchrecords"] = json!(records);
    league_val["statistics"] = json!(new_stats);
    league_val["nextround"] = json!(rnd + 1);

    if let Some(l) = st.league2.as_mut() {
        l["laststatus"] = json!(format!("league:{}", rnd));
    }

    json!({
        "matchtype": "league",
        "round": rnd,
        "leaguematches": played
    })
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

pub async fn ep_l2_me(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.read().await;
    Json(l2_me(&st))
}

pub async fn ep_l2_reward() -> impl IntoResponse {
    let list: Vec<Value> = (0..15)
        .map(|i| {
            json!({
                "ep": 20000 * (i + 1),
                "items": []
            })
        })
        .collect();
    Json(list)
}

pub async fn ep_l2_sponsor(Path(difficulty): Path<String>) -> impl IntoResponse {
    let _diff: u32 = difficulty.parse().unwrap_or(10);
    Json(json!({
        "result": true,
        "level": 1,
        "maxround": 2 * (LEAGUE_SIZE - 1) - 1,
        "sponsors": [
            { "_id": "1", "sponsorid": "1", "missions": [{ "mid": "2" }] },
            { "_id": "2", "sponsorid": "2", "missions": [{ "mid": "3" }] },
            { "_id": "3", "sponsorid": "3", "missions": [{ "mid": "4" }] }
        ]
    }))
}

pub async fn ep_l2_sponsor_achievement(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.read().await;
    let lg = st.league2.as_ref();

    let sponsor = lg
        .and_then(|l| l.get("sponsor"))
        .and_then(|s| s.as_str())
        .unwrap_or("1");

    let mid = sponsor_mid(sponsor);
    let my_team = st.my_teamid();

    let (records, maxround, rank) = if let Some(league_obj) = lg.and_then(|l| l.get("league")) {
        let recs: Vec<MatchRecord> = league_obj
            .get("matchrecords")
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .unwrap_or_default();
        let mr = league_obj
            .get("maxround")
            .and_then(|v| v.as_u64())
            .unwrap_or(17) as usize;
        let rk = league_obj
            .get("statistics")
            .and_then(|s| s.get(&my_team.to_string()))
            .and_then(|ts| ts.get("rank"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        (recs, mr, rk)
    } else {
        (vec![], 17, 1)
    };

    let (progress, achieved) = mission_progress(&records, my_team, mid, maxround, rank);

    Json(json!({
        "result": true,
        "level": 1,
        "missions": [{
            "mid": mid,
            "achievement": achieved,
            "progress": progress
        }]
    }))
}

#[derive(Debug, Deserialize)]
pub struct CalendarReq {
    pub difficulty: Option<u32>,
    pub sponsor: Option<String>,
}

pub async fn ep_l2_calendar(
    State(state): State<SharedState>,
    Json(req): Json<CalendarReq>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let teamid = st.my_teamid();
    let teams = default_league_teams(teamid);
    let diff = req.difficulty.unwrap_or(10);
    let sponsor = req.sponsor.unwrap_or_else(|| "1".into());
    let day = today_ms();
    let maxround = 2 * (teams.len() - 1) - 1;
    let initial_stats = league_stats(&teams, &[], None);

    st.league2 = Some(json!({
        "leagueid": 13,
        "difficultyselect": diff,
        "sponsor": sponsor,
        "laststatus": "opening",
        "openingday": day,
        "sponsoroffice": { "season": { "status": "started" } },
        "league": {
            "teams": teams,
            "maxround": maxround,
            "nextround": 0,
            "matchrecords": [],
            "statistics": initial_stats
        }
    }));

    let resources = l2_resources(&st);
    Json(json!({
        "result": true,
        "resources": resources
    }))
}

#[derive(Debug, Deserialize)]
pub struct MatchReq {
    pub matchtype: Option<String>,
    pub playtype: Option<String>,
    pub round: Option<usize>,
    pub home: Option<u32>,
    pub away: Option<u32>,
}

pub async fn ep_l2_match(
    State(state): State<SharedState>,
    Json(req): Json<MatchReq>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    if st.league2.is_none()
        || st
            .league2
            .as_ref()
            .and_then(|l| l.get("league"))
            .is_none()
    {
        return Json(json!({ "result": false, "reason": "LEAGUE_FAIL" }));
    }

    let rnd = req.round.unwrap_or(0);
    let home = req.home.unwrap_or_else(|| st.my_teamid());
    let away = req.away.unwrap_or(243);
    let match_id = format!("{:08x}{:016x}", Utc::now().timestamp(), 0x12345678u64);

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

    let match_body = json!({
        "id": match_id,
        "_id": match_id,
        "type": "League_2_league",
        "stage": "created",
        "gamesettings": {
            "home": true,
            "kit": ["home", "away"],
            "difficulty": 10
        },
        "side": {
            "left": match_side(home),
            "right": match_side(away)
        },
        "leagueparam": {
            "matchtype": "league",
            "round": rnd,
            "home": home,
            "away": away
        }
    });

    if let Some(lg) = st.league2.as_mut() {
        lg["pending"] = json!({
            "id": match_id,
            "round": rnd,
            "home": home,
            "away": away
        });
    }

    st.match_data = Some(match_body.clone());

    Json(json!({
        "result": true,
        "resources": [{
            "uri": format!("/match/{}", match_id),
            "body": match_body
        }]
    }))
}

pub async fn ep_l2_match_sim(
    State(state): State<SharedState>,
    Json(req): Json<MatchReq>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    if st.league2.is_none()
        || st
            .league2
            .as_ref()
            .and_then(|l| l.get("league"))
            .is_none()
    {
        return Json(json!({ "result": false, "reason": "LEAGUE_FAIL" }));
    }

    let rnd = req.round.unwrap_or(0);
    let home = req.home.unwrap_or_else(|| st.my_teamid());
    let away = req.away.unwrap_or(243);
    let my_team = st.my_teamid();

    let param = league_play_round(&mut st, rnd, None);

    let mine_match = param
        .get("leaguematches")
        .and_then(|m| m.as_array())
        .and_then(|arr| {
            arr.iter().find(|m| {
                let h = m.get("home").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let a = m.get("away").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                h == my_team || a == my_team
            })
        });

    let score = mine_match
        .and_then(|m| m.get("goals"))
        .cloned()
        .unwrap_or_else(|| json!({ "home": 1, "away": 0 }));

    let match_id = format!("{:08x}{:016x}", Utc::now().timestamp(), 0x87654321u64);
    let match_side = |tid: u32| {
        json!({
            "ready": true,
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

    let match_body = json!({
        "id": match_id,
        "type": "League_2_league",
        "stage": "closed",
        "simulated": true,
        "gamesettings": {
            "home": true,
            "kit": ["home", "away"],
            "difficulty": 10,
            "simulated": true
        },
        "side": {
            "left": match_side(home),
            "right": match_side(away)
        },
        "leagueparam": param,
        "result": { "home": { "score": score["home"] }, "away": { "score": score["away"] } }
    });

    let mut resources = vec![json!({
        "uri": format!("/match/{}", match_id),
        "body": match_body
    })];
    resources.extend(l2_resources(&st));

    Json(json!({
        "result": true,
        "leagueparam": param,
        "eventlist": [],
        "score": score,
        "resources": resources
    }))
}

pub async fn ep_l2_sleep(
    State(state): State<SharedState>,
    Json(req): Json<MatchReq>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let rnd = req.round.unwrap_or(0);
    let param = league_play_round(&mut st, rnd, None);
    let res = l2_resources(&st);
    Json(json!({
        "result": true,
        "leagueparam": param,
        "resources": res
    }))
}

pub async fn ep_l2_abandon(State(state): State<SharedState>) -> impl IntoResponse {
    let mut st = state.write().await;
    let cur_diff = st
        .league2
        .as_ref()
        .and_then(|l| l.get("difficultyselect"))
        .and_then(|v| v.as_u64())
        .unwrap_or(10);

    st.league2 = Some(json!({
        "difficultyselect": cur_diff
    }));

    let res = l2_resources(&st);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_l2_end(State(state): State<SharedState>) -> impl IntoResponse {
    let mut st = state.write().await;
    let my_team = st.my_teamid();

    let lg = match st.league2.as_mut() {
        Some(l) => l,
        None => return Json(json!({ "result": false, "reason": "LEAGUE_FAIL" })),
    };

    let league_val = match lg.get("league") {
        Some(l) => l.clone(),
        None => return Json(json!({ "result": false, "reason": "LEAGUE_FAIL" })),
    };

    let cur = lg
        .get("difficultyselect")
        .and_then(|v| v.as_u64())
        .unwrap_or(10) as u32;

    let stats = league_val
        .get("statistics")
        .and_then(|s| s.get(&my_team.to_string()))
        .cloned()
        .unwrap_or_else(|| json!({ "rank": 1, "goals": 10 }));

    let rank = stats.get("rank").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
    let goals = stats.get("goals").and_then(|v| v.as_u64()).unwrap_or(10) as u32;

    let nxt = if cur >= 20 {
        if rank <= 3 {
            (cur + 1).min(24)
        } else if rank > (LEAGUE_SIZE as u32 - 3) {
            (cur - 1).max(20)
        } else {
            cur
        }
    } else if rank <= 3 {
        (cur + 1).min(19)
    } else {
        cur
    };

    let sponsor_id = lg
        .get("sponsor")
        .and_then(|s| s.as_str())
        .unwrap_or("1")
        .to_string();
    let mid = sponsor_mid(&sponsor_id);

    let records: Vec<MatchRecord> = league_val
        .get("matchrecords")
        .and_then(|r| serde_json::from_value(r.clone()).ok())
        .unwrap_or_default();
    let maxround = league_val
        .get("maxround")
        .and_then(|v| v.as_u64())
        .unwrap_or(17) as usize;

    let (progress, done) = mission_progress(&records, my_team, mid, maxround, rank);

    let sponsor_ep = if done { 50000u64 } else { 0u64 };
    let leaders = season_leaders(&st, my_team, rank, goals);
    let leader_ep = 12000u64;
    let base_ep = ((LEAGUE_SIZE as u32 - rank + 1) * 2000) as u64;
    let total_ep = base_ep + sponsor_ep + leader_ep;
    let season_exp = (LEAGUE_SIZE as u32 - rank + 1) * 100;

    let prev_ep = st.my_coach().map(|c| c.ep).unwrap_or(1_000_000);
    let prev_coach_level = st
        .my_coach()
        .map(|c| c.levelinfo.clone())
        .unwrap_or_default();
    let cur_coach_level = json!({
        "level": prev_coach_level.level,
        "exp": prev_coach_level.exp + season_exp,
        "to": prev_coach_level.to
    });

    st.add_ep(total_ep as i64);

    if let Some(l) = st.league2.as_mut() {
        l["difficultyselect"] = json!(nxt);
        l["sponsoroffice"] = json!({ "season": { "status": "finished" } });
    }

    let out = json!({
        "result": true,
        "promotion": { "current": cur, "next": nxt },
        "rewards": {
            "coach": {
                "prev": prev_coach_level,
                "current": cur_coach_level,
                "reward": { "total": season_exp }
            },
            "money": {
                "prev": prev_ep,
                "reward": {
                    "total": total_ep,
                    "detail": {
                        "season": base_ep,
                        "sponsor": sponsor_ep,
                        "leader": leader_ep
                    }
                }
            }
        },
        "sponsorRewards": {
            "sponsorid": sponsor_id,
            "level": 1,
            "achievement": done,
            "missions": [{ "mid": mid, "achievement": done, "progress": progress }]
        },
        "leaderRewards": leaders,
        "seasonRewards": { "items": [] },
        "resources": l2_resources(&st)
    });

    Json(out)
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/league2/me", get(ep_l2_me))
        .route("/league2/reward", get(ep_l2_reward))
        .route("/league2/sponsor/{difficulty}", post(ep_l2_sponsor))
        .route("/league2/sponsor/achievement", put(ep_l2_sponsor_achievement))
        .route("/league2/calendar", post(ep_l2_calendar))
        .route("/league2/match", post(ep_l2_match))
        .route("/league2/match/sim", post(ep_l2_match_sim))
        .route("/league2/sleep", post(ep_l2_sleep))
        .route("/league2/abandon", put(ep_l2_abandon))
        .route("/league2/end", put(ep_l2_end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_league_round_properties() {
        let teams: Vec<u32> = (1..=10).collect();
        let rounds: Vec<_> = (0..(2 * (teams.len() - 1)))
            .map(|r| league_round(&teams, r))
            .collect();

        for pairs in &rounds {
            let mut round_teams = Vec::new();
            for &(h, a) in pairs {
                round_teams.push(h.unwrap());
                round_teams.push(a.unwrap());
            }
            round_teams.sort_unstable();
            assert_eq!(round_teams, teams);
        }

        let mut home_matches = HashMap::new();
        for pairs in &rounds {
            for &(h, a) in pairs {
                *home_matches.entry((h.unwrap(), a.unwrap())).or_insert(0) += 1;
            }
        }
        for (_, count) in home_matches {
            assert_eq!(count, 1);
        }
    }

    #[test]
    fn test_league_stats_ranking() {
        let teams = vec![1, 2];
        let records = vec![MatchRecord {
            r#type: "league".to_string(),
            matchtype: "league".to_string(),
            round: 0,
            home: 1,
            away: 2,
            goals: Goals { home: 3, away: 1 },
        }];

        let stats = league_stats(&teams, &records, None);
        assert_eq!(stats["1"].rank, 1);
        assert_eq!(stats["1"].wins, 1);
        assert_eq!(stats["1"].point, 3);
        assert_eq!(stats["2"].rank, 2);
        assert_eq!(stats["2"].losses, 1);
    }

    #[test]
    fn test_mission_progress() {
        let records = vec![
            MatchRecord {
                r#type: "league".to_string(),
                matchtype: "league".to_string(),
                round: 0,
                home: 1,
                away: 2,
                goals: Goals { home: 2, away: 0 },
            },
            MatchRecord {
                r#type: "league".to_string(),
                matchtype: "league".to_string(),
                round: 1,
                home: 3,
                away: 1,
                goals: Goals { home: 0, away: 1 },
            },
        ];

        // Mid 3: 3 wins in a row (target 3). Currently 2 wins in a row.
        let (prog3, done3) = mission_progress(&records, 1, "3", 17, 1);
        assert_eq!(prog3, vec![2]);
        assert!(!done3);

        // Mid 4: 2 clean sheets in a row (target 2). Both had 0 goals conceded.
        let (prog4, done4) = mission_progress(&records, 1, "4", 17, 1);
        assert_eq!(prog4, vec![2]);
        assert!(done4);
    }
}
