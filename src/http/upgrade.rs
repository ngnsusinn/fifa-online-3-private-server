use super::common::{
    card_price, sig3, EnhanceRestore, Player, SharedState, Team, GRADE_BOOST, POS_SUB,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use rand::Rng;
use serde_json::{json, Value};

pub const ENHANCE_RATE: [f64; 11] = [
    1.0, 1.0, 0.81, 0.64, 0.50, 0.26, 0.15, 0.07, 0.04, 0.02, 0.0,
];
pub const ENHANCE_DROP_FROM: u8 = 5;
pub const ES_BOOST: f64 = 0.10;
pub const ES_RESTORE: u32 = 3;
pub const EVENT_BOOST: f64 = 0.10;

pub fn boost_item_val(itemid: u32) -> f64 {
    match itemid {
        1000052 => 0.15, // Gold Boost
        1000053 => 0.10, // Silver Boost
        1000054 => 0.05, // Bronze Boost
        _ => 0.0,
    }
}

pub fn enhance_pair<'a>(
    team: &'a Team,
    a_id: &str,
    b_id: &str,
) -> Option<(&'a Player, &'a Player)> {
    if a_id == b_id {
        return None;
    }
    let pa = team.players.iter().find(|p| p._id == a_id)?;
    let pb = team.players.iter().find(|p| p._id == b_id)?;

    if pa.spid != pb.spid {
        return None;
    }
    if pa.grade >= 10 && pb.grade > 0 {
        return None; // +10 can only take a +0
    }
    Some((pa, pb))
}

pub fn enhance_grade(target_grade: u8, material_grade: u8) -> u8 {
    if material_grade == 0 {
        target_grade
    } else {
        (target_grade + 1).min(10)
    }
}

pub fn enhance_rate(target_grade: u8, material_grade: u8) -> f64 {
    if material_grade == 0 {
        return 1.0;
    }
    let tg = (target_grade as usize).min(10);
    let mg = material_grade as f64;
    let tg_f = target_grade as f64;

    let bonus = if material_grade <= 1 {
        1.0
    } else if mg >= tg_f * 2.0 / 3.0 {
        1.5
    } else if mg >= tg_f / 3.0 {
        1.25
    } else {
        1.1
    };

    (ENHANCE_RATE[tg] * bonus).min(1.0)
}

pub fn enhance_cost(target: &Player) -> u64 {
    let p = card_price(target.spid, target.grade_of());
    sig3(p / 50).max(1000)
}

pub fn enhance_es(target: &Player) -> u32 {
    10 * (target.grade_of() as u32 + 1)
}

pub fn enhance25_rate(
    target_grade: u8,
    material_grade: u8,
    use_point: bool,
    boost_val: f64,
    event_active: bool,
) -> f64 {
    let mut rate = enhance_rate(target_grade, material_grade);
    if material_grade > 0 {
        if use_point {
            rate += ES_BOOST;
        }
        rate += boost_val;
        if event_active && target_grade >= 1 && target_grade <= 9 {
            rate += EVENT_BOOST;
        }
    }
    rate.min(1.0)
}

pub fn merge_into(team: &mut Team, keep_id: &str, gone_id: &str) {
    let mut target_in_squad = false;
    let mut target_is_starter = false;
    let mut gone_in_squad = false;
    let mut gone_pos = 0;

    for s in &team.squad.positions {
        let sid = s.id.as_deref().unwrap_or("");
        if sid == keep_id {
            target_in_squad = true;
            if s.pos < POS_SUB {
                target_is_starter = true;
            }
        }
        if sid == gone_id {
            gone_in_squad = true;
            gone_pos = s.pos;
        }
    }

    if gone_in_squad && (!target_in_squad || (gone_pos < POS_SUB && !target_is_starter)) {
        if target_in_squad {
            team.squad
                .positions
                .retain(|s| s.id.as_deref() != Some(keep_id));
        }
        if let Some(pos) = team
            .squad
            .positions
            .iter_mut()
            .find(|s| s.id.as_deref() == Some(gone_id))
        {
            pos.id = Some(keep_id.to_string());
        }
        if let Some(kp) = team.players.iter_mut().find(|p| p._id == keep_id) {
            kp.team2 = None;
        }
        if team.squad.captain == gone_id {
            team.squad.captain = keep_id.to_string();
        }
        for k in &mut team.squad.kickers {
            if k == gone_id {
                *k = keep_id.to_string();
            }
        }
    }

    team.remove_player(gone_id);
}

pub fn pe2_staticvalues() -> Value {
    json!({
        "bonus": { "use": (ES_BOOST * 10000.0).round() as u32 },
        "simplify_step": { "use": 0 },
        "effect_price": {
            "0": 10_000_000_000u64,
            "1": 1_000_000_000u64,
            "2": 100_000_000u64,
            "3": 10_000_000u64,
            "4": 1_000_000u64
        }
    })
}

pub fn enhance_event() -> Value {
    json!({
        "eventid": 1001,
        "active": true,
        "reason": [],
        "condition": {
            "type": "enhance",
            "enhance": { "season": [] },
            "timeset": {
                "startdate": "20140101",
                "enddate": "20351231",
                "starttime": "0000",
                "endtime": "2359"
            },
            "targetuser": { "targetuser_igr": 255, "targetuser_vip": 255 }
        },
        "reward": [[{ "boosts": { "BOOST_PLUS_ENHANCE": (EVENT_BOOST * 100.0).round() as u32 } }]],
        "messages": {
            "main": {
                "name": "Event Upgrade",
                "description": "Peluang sukses upgrade +10% untuk target +1 sampai +9."
            }
        }
    })
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

pub async fn ep_enhance_preview(
    State(state): State<SharedState>,
    Path((target_id, material_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let st = state.read().await;
    let team = match st.my_team() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
    };

    let (pa, pb) = match enhance_pair(team, &target_id, &material_id) {
        Some(pair) => pair,
        None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
    };

    let next_g = enhance_grade(pa.grade_of(), pb.grade_of());
    let rate = enhance_rate(pa.grade_of(), pb.grade_of());
    let cost = enhance_cost(pa);
    let price = card_price(pa.spid, next_g);

    Json(json!({
        "result": true,
        "grade": next_g,
        "levelinfo": pa.levelinfo.clone().unwrap_or_default(),
        "price": price,
        "enhancepay": cost,
        "probability": (rate * 100.0).round() as u32
    }))
}

pub async fn ep_enhance(
    State(state): State<SharedState>,
    Path((target_id, material_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let slot = st.default_slot();

    let cost;
    let before;
    let next_g;
    let rate;
    let spid;

    {
        let team = match st.my_team() {
            Some(t) => t,
            None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
        };

        let (pa, pb) = match enhance_pair(team, &target_id, &material_id) {
            Some(pair) => pair,
            None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
        };

        cost = enhance_cost(pa);
        before = pa.grade_of();
        next_g = enhance_grade(before, pb.grade_of());
        rate = enhance_rate(before, pb.grade_of());
        spid = pa.spid;
    }

    let coach_ep = st.my_coach().map(|c| c.ep).unwrap_or(0);
    if coach_ep < cost {
        return Json(json!({ "result": false, "reason": "Invalid target ep price" }));
    }

    st.add_ep(-(cost as i64));

    let mut rng = rand::thread_rng();
    let success = rng.gen::<f64>() < rate;

    let after = if success {
        next_g
    } else if before >= ENHANCE_DROP_FROM {
        before - 1
    } else {
        before
    };

    if let Some(team) = st.my_team_mut() {
        if let Some(pa) = team.players.iter_mut().find(|p| p._id == target_id) {
            pa.grade = after;
        }
        merge_into(team, &target_id, &material_id);
    }

    let attrib_plus = (GRADE_BOOST[after as usize] as i32) - (GRADE_BOOST[before as usize] as i32);
    let rprice = card_price(spid, after);
    let resources = st.coach_me_resources(slot);

    Json(json!({
        "result": true,
        "success": success,
        "grade": after,
        "rgrade": after,
        "rprice": rprice,
        "attribPlus": attrib_plus,
        "resources": resources
    }))
}

pub async fn ep_enhance25_preview(
    State(state): State<SharedState>,
    Path((target_id, material_id, use_point, item_id, event_id)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> impl IntoResponse {
    let st = state.read().await;
    let team = match st.my_team() {
        Some(t) => t,
        None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
    };

    let (pa, pb) = match enhance_pair(team, &target_id, &material_id) {
        Some(pair) => pair,
        None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
    };

    let tg = pa.grade_of();
    let mg = pb.grade_of();
    let next_g = enhance_grade(tg, mg);

    let use_p = use_point == "1";
    let item_num: u32 = item_id.parse().unwrap_or(0);
    let boost_val = boost_item_val(item_num);
    let ev_num: i64 = event_id.parse().unwrap_or(-1);
    let event_active = ev_num == 1001;

    let rate = enhance25_rate(tg, mg, use_p, boost_val, event_active);
    let cost = enhance_cost(pa);
    let need = enhance_es(pa);
    let price = card_price(pa.spid, next_g);

    Json(json!({
        "result": true,
        "grade": next_g,
        "levelinfo": pa.levelinfo.clone().unwrap_or_default(),
        "price": price,
        "probability": (rate * 100.0).round() as u32,
        "enhancepay": cost,
        "needes": need
    }))
}

pub async fn ep_enhance25(
    State(state): State<SharedState>,
    Path((target_id, material_id, use_point, item_id, event_id)): Path<(
        String,
        String,
        String,
        String,
        String,
    )>,
) -> impl IntoResponse {
    let mut st = state.write().await;
    let slot = st.default_slot();

    let cost;
    let need;
    let before;
    let next_g;
    let spid;
    let mg;

    {
        let team = match st.my_team() {
            Some(t) => t,
            None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
        };

        let (pa, pb) = match enhance_pair(team, &target_id, &material_id) {
            Some(pair) => pair,
            None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
        };

        cost = enhance_cost(pa);
        need = enhance_es(pa);
        before = pa.grade_of();
        mg = pb.grade_of();
        next_g = enhance_grade(before, mg);
        spid = pa.spid;
    }

    let boostable = mg > 0;
    let use_p = use_point == "1" && boostable;
    let item_num: i64 = item_id.parse().unwrap_or(-1);

    let has_item = if boostable && item_num != -1 {
        let it_id = item_num as u32;
        st.my_coach()
            .map(|c| c.items.iter().any(|i| i.itemid == it_id && i.remains > 0))
            .unwrap_or(false)
    } else {
        false
    };

    if boostable && item_num != -1 && !has_item {
        return Json(json!({ "result": false, "reason": "expired item" }));
    }

    let coach_ep = st.my_coach().map(|c| c.ep).unwrap_or(0);
    let coach_es = st.my_coach().map(|c| c.es).unwrap_or(0);

    if coach_ep < cost {
        return Json(json!({ "result": false, "reason": "Invalid target ep price" }));
    }
    if use_p && coach_es < need {
        return Json(json!({ "result": false, "reason": "NOT_ENOUGH_ES" }));
    }

    let boost_val = if has_item {
        boost_item_val(item_num as u32)
    } else {
        0.0
    };
    let ev_num: i64 = event_id.parse().unwrap_or(-1);
    let rate = enhance25_rate(before, mg, use_p, boost_val, ev_num == 1001);

    st.add_ep(-(cost as i64));
    if use_p {
        st.add_es(-(need as i32));
    }

    if has_item {
        let it_id = item_num as u32;
        if let Some(c) = st.my_coach_mut() {
            if let Some(it) = c.items.iter_mut().find(|i| i.itemid == it_id && i.remains > 0) {
                it.remains -= 1;
            }
            c.items.retain(|i| i.remains > 0);
        }
    }

    let mut rng = rand::thread_rng();
    let success = rng.gen::<f64>() < rate;

    let after = if success {
        next_g
    } else if before >= ENHANCE_DROP_FROM {
        before - 1
    } else {
        before
    };

    if let Some(team) = st.my_team_mut() {
        if let Some(pa) = team.players.iter_mut().find(|p| p._id == target_id) {
            pa.grade = after;
        }
        merge_into(team, &target_id, &material_id);
    }

    let grant = if success { 0 } else { need };
    st.add_es(grant as i32);

    st.enhance_restore = None;
    let restore_price = ES_RESTORE * need;
    if after < before {
        st.enhance_restore = Some(EnhanceRestore {
            id: target_id.clone(),
            grade: before,
            price: restore_price,
        });
    }

    let attrib_plus = (GRADE_BOOST[after as usize] as i32) - (GRADE_BOOST[before as usize] as i32);
    let rprice = card_price(spid, after);
    let resources = st.coach_me_resources(slot);

    Json(json!({
        "result": true,
        "success": success,
        "grade": after,
        "rgrade": after,
        "rprice": rprice,
        "attribPlus": attrib_plus,
        "grantes": grant,
        "usepoint": {
            "currency": "ES",
            "price": restore_price
        },
        "getitems": [],
        "resources": resources
    }))
}

pub async fn ep_enhance25_restore(State(state): State<SharedState>) -> impl IntoResponse {
    let mut st = state.write().await;
    let slot = st.default_slot();

    let restore = match st.enhance_restore.clone() {
        Some(r) => r,
        None => return Json(json!({ "result": false, "reason": "INVALID_PARAM" })),
    };

    let coach_es = st.my_coach().map(|c| c.es).unwrap_or(0);
    if coach_es < restore.price {
        return Json(json!({ "result": false, "reason": "NOT_ENOUGH_ES" }));
    }

    st.add_es(-(restore.price as i32));

    let mut before = 0;
    let mut spid = 0;
    if let Some(team) = st.my_team_mut() {
        if let Some(p) = team.players.iter_mut().find(|p| p._id == restore.id) {
            before = p.grade_of();
            p.grade = restore.grade;
            spid = p.spid;
        }
    }

    st.enhance_restore = None;

    let attrib_plus = (GRADE_BOOST[restore.grade as usize] as i32) - (GRADE_BOOST[before as usize] as i32);
    let rprice = card_price(spid, restore.grade);
    let resources = st.coach_me_resources(slot);

    Json(json!({
        "result": true,
        "success": false,
        "grade": restore.grade,
        "rgrade": restore.grade,
        "rprice": rprice,
        "attribPlus": attrib_plus,
        "grantes": 0,
        "usepoint": {
            "currency": "ES",
            "price": restore.price
        },
        "getitems": [],
        "resources": resources
    }))
}

pub async fn ep_pe2_staticvalues() -> impl IntoResponse {
    Json(pe2_staticvalues())
}

pub async fn ep_events() -> impl IntoResponse {
    Json(json!({
        "events": [enhance_event()]
    }))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route(
            "/player/{target}/enhance/{material}/preview",
            get(ep_enhance_preview),
        )
        .route(
            "/player/{target}/enhance/{material}",
            put(ep_enhance),
        )
        .route(
            "/player/{target}/enhance2.5/{material}/preview/{use_point}/{item_id}/{event_id}",
            get(ep_enhance25_preview),
        )
        .route(
            "/player/{target}/enhance2.5/{material}/{use_point}/{item_id}/{event_id}",
            put(ep_enhance25),
        )
        .route("/player/enhance2.5/restore", put(ep_enhance25_restore))
        .route("/static/db/PE2STATICVALUES", get(ep_pe2_staticvalues))
        .route("/events", get(ep_events))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_rates_and_bonus() {
        // Base rates
        assert_eq!(enhance_rate(1, 1), 1.0);
        assert_eq!(enhance_rate(2, 1), 0.81);

        // +6 target: +1 Kosong (1.0x), +2 Normal (1.25x), +4 High (1.5x)
        let r6_1 = enhance_rate(6, 1);
        let r6_2 = enhance_rate(6, 2);
        let r6_4 = enhance_rate(6, 4);

        assert_eq!((r6_1 * 10000.0).round() as u32, 1500); // 0.15
        assert_eq!((r6_2 * 10000.0).round() as u32, 1875); // 0.1875
        assert_eq!((r6_4 * 10000.0).round() as u32, 2250); // 0.225

        // +0 material always 100%
        assert_eq!(enhance_rate(10, 0), 1.0);
        assert_eq!(enhance_grade(10, 0), 10);
        assert_eq!(enhance_grade(4, 0), 4);
    }

    #[test]
    fn test_enhance25_points_and_restore() {
        let p = Player {
            _id: "test".into(),
            spid: 1001,
            expandme: "player".into(),
            grade: 6,
            jersey: 10,
            jerseyname: "".into(),
            team2: None,
            levelinfo: None,
        };

        assert_eq!(enhance_es(&p), 70); // 10 * (6 + 1)
        assert_eq!(ES_RESTORE * enhance_es(&p), 210);
    }

    #[test]
    fn test_boost_items_values() {
        assert_eq!(boost_item_val(1000052), 0.15); // Gold
        assert_eq!(boost_item_val(1000053), 0.10); // Silver
        assert_eq!(boost_item_val(1000054), 0.05); // Bronze
        assert_eq!(boost_item_val(999), 0.0);
    }
}
