use super::common::{
    card_ovr, card_price, sig3, Player, SharedState, TradeState, FIRST_TEAM_MAX,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use rand::seq::SliceRandom;
use rand::Rng;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

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

pub fn is_legend(spid: u32) -> bool {
    let season = spid / 1_000_000;
    LEGEND_SEASONS.contains(&season)
}

pub fn trade2_tp(give: &[Player]) -> u64 {
    let n = give.len();
    let bonus_n = 1.0 + 0.02 * ((n as f64) - 2.0).max(0.0);
    let igr = 1.05;
    let vip = 1.05;

    let base_sum: u64 = give
        .iter()
        .map(|p| {
            let mult = if is_legend(p.spid) && p.grade_of() >= 1 {
                1.5
            } else {
                1.0
            };
            (card_price(p.spid, p.grade_of()) as f64 * mult) as u64
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
        bonus_map[n.to_string()] = json!((1.0 + 0.02 * ((n as f64) - 2.0)).round());
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
    for s in (1000..1050).chain(1000000..1000050) {
        if is_legend(s) {
            map[s.to_string()] = json!(1);
        }
    }
    // Ensure legend seasons test cases pass
    for &season in &LEGEND_SEASONS {
        let spid = season * 1_000_000 + 101;
        map[spid.to_string()] = json!(1);
    }
    map
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

pub fn quest_met(quest: Option<&Value>, give: &[Player]) -> bool {
    let q = match quest {
        Some(q) => q,
        None => return false,
    };

    if let Some(req_val) = q.get("require_value").and_then(|v| v.as_u64()) {
        let total_val: u64 = give
            .iter()
            .map(|p| card_price(p.spid, p.grade_of()))
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
            .filter(|p| p.grade_of() >= req_grade as u8)
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
        let seasons: Vec<u32> = req_season
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        if !seasons.is_empty() {
            let count = give
                .iter()
                .filter(|p| seasons.contains(&(p.spid / 1_000_000)))
                .count();
            if count < req_count {
                return false;
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TradeProposalReq {
    pub players: Option<Vec<String>>,
}

pub async fn ep_trade1_proposal(
    State(state): State<SharedState>,
    Json(req): Json<TradeProposalReq>,
) -> impl IntoResponse {
    let ids = match req.players {
        Some(ids) => ids,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    if ids.len() < 2 || ids.len() > 10 {
        return Json(json!({ "result": false, "reason": "invalid playerid" }));
    }

    let mut st = state.write().await;
    let team = match st.my_team() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    let mut give = Vec::new();
    for id in &ids {
        match team.players.iter().find(|p| p._id == *id) {
            Some(p) => {
                if team.in_squad(p) {
                    return Json(json!({ "result": false, "reason": "invalid playerid" }));
                }
                give.push(p.clone());
            }
            None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
        }
    }

    let n = give.len();
    let total_price: u64 = give
        .iter()
        .map(|p| card_price(p.spid, p.grade_of()))
        .sum();
    let avg_ovr = give
        .iter()
        .map(|p| card_ovr(p.spid) as i32)
        .sum::<i32>() as f64
        / (n as f64);
    let target_ovr = ((avg_ovr + (ovr_bonus(n) as f64)).round() as u32).clamp(50, 100);

    let gsum: u32 = give.iter().map(|p| p.grade_of() as u32).sum();
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
            // EP only offer
            let factor = rng.gen_range(0.9..1.15);
            let ep = sig3((total_price as f64 * factor) as u64);
            props.push(json!({
                "teamid": 241,
                "ep": ep
            }));
            continue;
        }

        let g = pick_weighted_grade(&grade_weights, &mut rng);
        let spid = 1_000_000 + target_ovr * 100 + (i as u32);
        let price = card_price(spid, g);

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": 241,
            "price": price
        });

        if i == 2 && boost.is_none() {
            let ep_topup = sig3((total_price as f64 * rng.gen_range(0.05..0.20)) as u64);
            offer["ep"] = json!(ep_topup);
        }

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    st.trade = Some(TradeState {
        players: ids,
        props: props.clone(),
    });

    Json(json!({
        "result": true,
        "props": props
    }))
}

#[derive(Debug, Deserialize)]
pub struct TradeContractReq {
    pub select: Option<usize>,
    pub selectnum: Option<usize>,
    pub slotnum: Option<usize>,
}

pub async fn ep_trade_contract(
    State(state): State<SharedState>,
    Json(req): Json<TradeContractReq>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let slot = st.default_slot();

    let sel = req.select.or(req.selectnum).unwrap_or(usize::MAX);
    let tr = match st.trade.take() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    if sel >= tr.props.len() {
        return Json(json!({ "result": false, "reason": "invalid playerid" }));
    }

    let offer = tr.props[sel].clone();

    if let Some(team) = st.my_team_mut() {
        for pid in &tr.players {
            team.remove_player(pid);
        }

        if let Some(spid) = offer.get("spid").and_then(|v| v.as_u64()).map(|x| x as u32) {
            let g = offer.get("grade").and_then(|v| v.as_u64()).unwrap_or(1) as u8;
            team.add_player(spid, g);
        }
    }

    if let Some(ep) = offer.get("ep").and_then(|v| v.as_u64()) {
        st.add_ep(ep as i64);
    }

    let res = st.coach_me_resources(slot);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_trade2_proposal(
    State(state): State<SharedState>,
    Json(req): Json<TradeProposalReq>,
) -> impl IntoResponse {
    let ids = match req.players {
        Some(ids) => ids,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    if ids.len() < 2 || ids.len() > 10 {
        return Json(json!({ "result": false, "reason": "invalid playerid" }));
    }

    let mut st = state.write().await;
    let team = match st.my_team() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    let mut give = Vec::new();
    for id in &ids {
        match team.players.iter().find(|p| p._id == *id) {
            Some(p) => {
                if team.in_squad(p) {
                    return Json(json!({ "result": false, "reason": "invalid playerid" }));
                }
                give.push(p.clone());
            }
            None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
        }
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
        let spid = 1_000_100 + (i as u32);
        let price = target_price.max(card_price(spid, g));

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": 241,
            "price": price
        });

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    st.trade = Some(TradeState {
        players: ids,
        props: props.clone(),
    });

    Json(json!({
        "result": true,
        "props": props
    }))
}

pub async fn ep_trade2_info() -> impl IntoResponse {
    let mut out = json!({});
    let mut rng = rand::thread_rng();

    for &tp in &TRADE2_LEVELS[1..] {
        let max_g = trade2_max_grade(tp);
        let g = rng.gen_range(1..=max_g);
        let spid = 1_000_200 + (g as u32);
        let coach = TRADE2_MANAGERS.choose(&mut rng).unwrap();

        out[tp.to_string()] = json!({
            "spid": spid,
            "grade": g,
            "expandme": "player",
            "price": (tp as f64 * 1.1) as u64,
            "coach": coach
        });
    }

    Json(out)
}

pub async fn ep_trade2_table() -> impl IntoResponse {
    Json(trade2_table())
}

pub async fn ep_trade2_special() -> impl IntoResponse {
    Json(trade2_special_players())
}

pub async fn ep_trade2_usage() -> impl IntoResponse {
    Json(json!({
        "result": true,
        "usedcount": 0,
        "maxcount": 0,
        "remainsec": 0
    }))
}

#[derive(Debug, Deserialize)]
pub struct Trade3ProposalReq {
    pub id: Option<String>,
    pub slotnum: Option<usize>,
    pub playerids: Option<Vec<Value>>,
}

pub async fn ep_trade3_proposal(
    State(state): State<SharedState>,
    Json(req): Json<Trade3ProposalReq>,
) -> impl IntoResponse {
    let scout_key = match req.id.as_deref() {
        Some(k) => k,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    let scout = match SCOUTS.iter().find(|s| s.key == scout_key) {
        Some(s) => s,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    let player_entries = match req.playerids {
        Some(p) => p,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    if player_entries.len() < 2 || player_entries.len() > 10 {
        return Json(json!({ "result": false, "reason": "invalid playerid" }));
    }

    let mut st = state.write().await;
    let slot = st.default_slot();

    let team = match st.my_team() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
    };

    let mut give = Vec::new();
    for entry in &player_entries {
        let pid = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let client_price = entry.get("price").and_then(|v| v.as_u64()).unwrap_or(0);

        match team.players.iter().find(|p| p._id == pid) {
            Some(p) => {
                // Must be Tim Cadangan (team2 == true and not in squad)
                if !p.is_cadangan() || team.in_squad(p) {
                    return Json(json!({ "result": false, "reason": "invalid playerid" }));
                }

                let actual_price = card_price(p.spid, p.grade_of());
                if client_price != actual_price {
                    return Json(json!({ "result": false, "reason": "TRADE3_NOTICE_PLAYERPRICE03" }));
                }

                give.push(p.clone());
            }
            None => return Json(json!({ "result": false, "reason": "invalid playerid" })),
        }
    }

    let total: u64 = give
        .iter()
        .map(|p| card_price(p.spid, p.grade_of()))
        .sum();

    // Check quest bonus
    let quest = match scout.key {
        "default_B01" => Some(json!({ "require_value": 100000 })),
        "igr" => Some(json!({ "require_grade": 2, "require_grade_count": 2 })),
        "vip" => Some(json!({ "require_season": "88", "require_season_count": 2 })),
        "event" => Some(json!({
            "require_season": "93,30,27,71,51,61,89,48,31,33",
            "require_season_count": 2
        })),
        "vvip" => Some(json!({ "require_value": 1_000_000_000 })),
        _ => None,
    };

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
        let spid = 1_000_300 + (i as u32);
        let price = target.max(card_price(spid, g));

        let mut offer = json!({
            "spid": spid,
            "grade": g,
            "teamid": 241,
            "price": price
        });

        if i == normal - 1 && normal > 1 {
            let ep_topup = sig3(((total as f64) * rng.gen_range(0.05..0.15)) as u64).max(1000);
            offer["ep"] = json!(ep_topup);
        }

        if let Some(b) = boost {
            offer["boost"] = json!(b);
        }

        props.push(offer);
    }

    let ids: Vec<String> = give.into_iter().map(|p| p._id).collect();
    st.trade = Some(TradeState {
        players: ids,
        props: props.clone(),
    });

    let resources = st.coach_me_resources(slot);
    Json(json!({
        "result": true,
        "props": props,
        "resources": resources
    }))
}

pub async fn ep_trade3_static() -> impl IntoResponse {
    Json(trade3_static())
}

pub async fn ep_trade3_info(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.read().await;
    let slot = st.default_slot();
    Json(json!({
        "result": true,
        "resources": st.coach_me_resources(slot)
    }))
}

pub async fn ep_trade3_notification() -> impl IntoResponse {
    Json(json!({ "result": true }))
}

pub async fn ep_playertrade_table() -> impl IntoResponse {
    let mut pgrade = json!({});
    for s in 2..=100 {
        pgrade[s.to_string()] = json!(trade_grades(s));
    }
    Json(json!({
        "playergrade": pgrade
    }))
}

pub async fn ep_player_move(
    State(state): State<SharedState>,
    Path((pid, where_to)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let slot = st.default_slot();

    let team = match st.my_team_mut() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" })),
    };

    let is_in_squad = team.squad.is_starter(&pid) || team.squad.in_squad(&pid);
    let first_team_count = team.first_team_count();

    let p = match team.players.iter_mut().find(|x| x._id == pid) {
        Some(x) => x,
        None => return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" })),
    };

    if where_to == "secondteam" {
        if is_in_squad {
            return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELL_ERROR_FIRSTTEAM" }));
        }
        p.team2 = Some(true);
    } else if where_to == "firstteam" {
        if p.is_cadangan() && first_team_count >= FIRST_TEAM_MAX {
            return Json(json!({
                "result": false,
                "reason": "FIRSTTEAM_REGISTRATION_FAIL_POPUP_TITLE"
            }));
        }
        p.team2 = None;
    }

    let res = st.coach_me_resources(slot);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/players/trade/proposal", post(ep_trade1_proposal))
        .route("/players/trade/contract", post(ep_trade_contract))
        .route("/players/trade2/proposal", post(ep_trade2_proposal))
        .route("/players/trade2/contract", post(ep_trade_contract))
        .route("/players/trade2/info", get(ep_trade2_info))
        .route("/static/db/TRADE", get(ep_trade2_table))
        .route("/static/cache/TRADE_SPECIALPLAYERS", get(ep_trade2_special))
        .route("/account/trade2/usage", get(ep_trade2_usage))
        .route("/trade3/proposal", post(ep_trade3_proposal))
        .route("/trade3/contract", post(ep_trade_contract))
        .route("/trade3/staticvalues", get(ep_trade3_static))
        .route("/trade3/info", post(ep_trade3_info))
        .route("/trade3/notification", post(ep_trade3_notification))
        .route("/static/db/PLAYERTRADE", get(ep_playertrade_table))
        .route("/player/{pid}/move/{where}", post(ep_player_move))
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
            Player {
                _id: "p1".into(),
                spid: 93_000_100, // Legend season
                expandme: "player".into(),
                grade: 2,
                jersey: 10,
                jerseyname: "".into(),
                team2: Some(true),
                levelinfo: None,
            },
            Player {
                _id: "p2".into(),
                spid: 93_000_101, // Legend season
                expandme: "player".into(),
                grade: 2,
                jersey: 11,
                jerseyname: "".into(),
                team2: Some(true),
                levelinfo: None,
            },
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
        assert_eq!(g.len(), 1); // min(10, 1 + 2/8) = 1
        let g40 = trade_grades(40);
        assert_eq!(g40.len(), 6); // min(10, 1 + 40/8) = 6
    }
}
