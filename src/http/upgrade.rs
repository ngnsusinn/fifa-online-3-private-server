use axum::{
    extract::Path,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use rand::Rng;
use serde_json::{json, Value};

pub const ENHANCE_RATE: [f64; 11] = [
    1.0, 1.0, 0.81, 0.64, 0.50, 0.26, 0.15, 0.07, 0.04, 0.02, 0.0,
];
pub const ENHANCE_DROP_FROM: usize = 5;
pub const ES_BOOST: f64 = 0.10;
pub const ES_RESTORE: u32 = 3;
pub const EVENT_BOOST: f64 = 0.10;
pub const GRADE_BOOST: [u32; 11] = [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24];

pub fn boost_item_val(itemid: u64) -> f64 {
    match itemid {
        1000052 => 0.15, // Gold Boost
        1000053 => 0.10, // Silver Boost
        1000054 => 0.05, // Bronze Boost
        _ => 0.0,
    }
}

pub fn enhance_pair<'a>(
    team: &'a Value,
    a_id: &str,
    b_id: &str,
) -> Option<(&'a Value, &'a Value)> {
    if a_id == b_id {
        return None;
    }
    let players = team.get("players").and_then(|v| v.as_array())?;
    let pa = players.iter().find(|p| crate::state::iid(p) == a_id)?;
    let pb = players.iter().find(|p| crate::state::iid(p) == b_id)?;

    let spid_a = pa.get("spid").and_then(|v| v.as_u64())?;
    let spid_b = pb.get("spid").and_then(|v| v.as_u64())?;
    if spid_a != spid_b {
        return None;
    }
    let ga = crate::state::grade_of(pa);
    let gb = crate::state::grade_of(pb);
    if ga >= 10 && gb > 0 {
        return None; // +10 can only take a +0
    }
    Some((pa, pb))
}

pub fn enhance_grade(target_grade: usize, material_grade: usize) -> usize {
    if material_grade == 0 {
        target_grade
    } else {
        (target_grade + 1).min(10)
    }
}

pub fn enhance_rate(target_grade: usize, material_grade: usize) -> f64 {
    if material_grade == 0 {
        return 1.0;
    }
    let tg = target_grade.min(10);
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

pub fn enhance_cost(spid: u64, grade: usize) -> u64 {
    let p = crate::state::card_price(spid, grade);
    crate::state::sig3(p / 50).max(1000)
}

pub fn enhance_es(grade: usize) -> u32 {
    10 * (grade as u32 + 1)
}

pub fn enhance25_rate(
    target_grade: usize,
    material_grade: usize,
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

pub fn merge_into(team: &mut Value, keep_id: &str, gone_id: &str) {
    let pos_sub = crate::state::POS_SUB;
    let mut target_in_squad = false;
    let mut target_is_starter = false;
    let mut gone_in_squad = false;
    let mut gone_pos = 0;

    if let Some(positions) = team
        .get("squad")
        .and_then(|sq| sq.get("positions"))
        .and_then(|v| v.as_array())
    {
        for s in positions {
            let sid = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let pos = s.get("pos").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            if sid == keep_id {
                target_in_squad = true;
                if pos < pos_sub {
                    target_is_starter = true;
                }
            }
            if sid == gone_id {
                gone_in_squad = true;
                gone_pos = pos;
            }
        }
    }

    if gone_in_squad && (!target_in_squad || (gone_pos < pos_sub && !target_is_starter)) {
        if let Some(sq) = team.get_mut("squad") {
            if let Some(positions) = sq.get_mut("positions").and_then(|v| v.as_array_mut()) {
                if target_in_squad {
                    positions.retain(|s| s.get("id").and_then(|v| v.as_str()) != Some(keep_id));
                }
                if let Some(pos) = positions
                    .iter_mut()
                    .find(|s| s.get("id").and_then(|v| v.as_str()) == Some(gone_id))
                {
                    pos["id"] = json!(keep_id);
                }
            }
            if sq.get("captain").and_then(|v| v.as_str()) == Some(gone_id) {
                sq["captain"] = json!(keep_id);
            }
            if let Some(kickers) = sq.get_mut("kickers").and_then(|v| v.as_array_mut()) {
                for k in kickers {
                    if k.as_str() == Some(gone_id) {
                        *k = json!(keep_id);
                    }
                }
            }
        }
        if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
            if let Some(kp) = players.iter_mut().find(|p| crate::state::iid(p) == keep_id) {
                if let Some(obj) = kp.as_object_mut() {
                    obj.remove("team2");
                }
            }
        }
    }

    crate::state::remove_player(team, gone_id);
}

pub fn pe2_staticvalues() -> Value {
    json!({
        "bonus": { "use": 1000 },
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
        "reward": [[{ "boosts": { "BOOST_PLUS_ENHANCE": 10 } }]],
        "messages": {
            "main": {
                "name": "Event Upgrade",
                "description": "Peluang sukses upgrade +10% cho mục tiêu từ +1 đến +9."
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Business Logic Handlers
// ---------------------------------------------------------------------------

pub fn handle_pe2_staticvalues() -> Value {
    pe2_staticvalues()
}

pub fn handle_enhance_preview(st: &Value, target_id: &str, material_id: &str) -> Value {
    let team = crate::state::my_team(st);
    let (pa, pb) = match enhance_pair(team, target_id, material_id) {
        Some(pair) => pair,
        None => return json!({ "result": false, "reason": "INVALID_PARAM" }),
    };

    let spid = pa.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
    let tg = crate::state::grade_of(pa);
    let mg = crate::state::grade_of(pb);
    let next_g = enhance_grade(tg, mg);
    let rate = enhance_rate(tg, mg);
    let cost = enhance_cost(spid, tg);
    let price = crate::state::card_price(spid, next_g);
    let levelinfo = pa.get("levelinfo").cloned().unwrap_or(json!({ "level": 1, "exp": 0 }));

    json!({
        "result": true,
        "grade": next_g,
        "levelinfo": levelinfo,
        "price": price,
        "enhancepay": cost,
        "probability": (rate * 100.0).round() as u32
    })
}

pub fn handle_enhance(
    st: &mut Value,
    _slot: usize,
    target_id: &str,
    material_id: &str,
    coach_me: Value,
) -> Value {
    let (spid, tg, _mg, next_g, rate, cost) = {
        let team = crate::state::my_team(st);
        let (pa, pb) = match enhance_pair(team, target_id, material_id) {
            Some(pair) => pair,
            None => return json!({ "result": false, "reason": "INVALID_PARAM" }),
        };
        let spid = pa.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        let tg = crate::state::grade_of(pa);
        let mg = crate::state::grade_of(pb);
        let next_g = enhance_grade(tg, mg);
        let rate = enhance_rate(tg, mg);
        let cost = enhance_cost(spid, tg);
        (spid, tg, mg, next_g, rate, cost)
    };

    let coach_ep = crate::state::my_coach(st)
        .get("ep")
        .and_then(|v| v.as_i64())
        .unwrap_or(crate::state::START_EP);
    if coach_ep < cost as i64 {
        return json!({ "result": false, "reason": "Invalid target ep price" });
    }

    crate::state::add_ep(st, -(cost as i64));

    let mut rng = rand::thread_rng();
    let success = rng.gen::<f64>() < rate;

    let after = if success {
        next_g
    } else if tg >= ENHANCE_DROP_FROM {
        tg - 1
    } else {
        tg
    };

    let team = crate::state::my_team_mut(st);
    if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
        if let Some(pa) = players.iter_mut().find(|p| crate::state::iid(p) == target_id) {
            pa["grade"] = json!(after);
        }
    }
    merge_into(team, target_id, material_id);

    let attrib_plus = (GRADE_BOOST[after] as i32) - (GRADE_BOOST[tg] as i32);
    let rprice = crate::state::card_price(spid, after);

    json!({
        "result": true,
        "success": success,
        "grade": after,
        "rgrade": after,
        "rprice": rprice,
        "attribPlus": attrib_plus,
        "resources": [
            {
                "uri": "/coach/me",
                "body": coach_me
            }
        ]
    })
}

pub fn handle_enhance25_preview(
    st: &Value,
    target_id: &str,
    material_id: &str,
    use_point: &str,
    item_id: &str,
    event_id: &str,
) -> Value {
    let team = crate::state::my_team(st);
    let (pa, pb) = match enhance_pair(team, target_id, material_id) {
        Some(pair) => pair,
        None => return json!({ "result": false, "reason": "INVALID_PARAM" }),
    };

    let spid = pa.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
    let tg = crate::state::grade_of(pa);
    let mg = crate::state::grade_of(pb);
    let next_g = enhance_grade(tg, mg);

    let use_p = use_point == "1";
    let item_num: u64 = item_id.parse().unwrap_or(0);
    let boost_val = boost_item_val(item_num);
    let ev_num: i64 = event_id.parse().unwrap_or(-1);
    let event_active = ev_num == 1001;

    let rate = enhance25_rate(tg, mg, use_p, boost_val, event_active);
    let cost = enhance_cost(spid, tg);
    let need = enhance_es(tg);
    let price = crate::state::card_price(spid, next_g);
    let levelinfo = pa.get("levelinfo").cloned().unwrap_or(json!({ "level": 1, "exp": 0 }));

    json!({
        "result": true,
        "grade": next_g,
        "levelinfo": levelinfo,
        "price": price,
        "probability": (rate * 100.0).round() as u32,
        "enhancepay": cost,
        "needes": need
    })
}

pub fn handle_enhance25(
    st: &mut Value,
    _slot: usize,
    target_id: &str,
    material_id: &str,
    use_point: &str,
    item_id: &str,
    event_id: &str,
    coach_me: Value,
) -> Value {
    let (spid, tg, mg, next_g, cost, need) = {
        let team = crate::state::my_team(st);
        let (pa, pb) = match enhance_pair(team, target_id, material_id) {
            Some(pair) => pair,
            None => return json!({ "result": false, "reason": "INVALID_PARAM" }),
        };
        let spid = pa.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        let tg = crate::state::grade_of(pa);
        let mg = crate::state::grade_of(pb);
        let next_g = enhance_grade(tg, mg);
        let cost = enhance_cost(spid, tg);
        let need = enhance_es(tg);
        (spid, tg, mg, next_g, cost, need)
    };

    let boostable = mg > 0;
    let use_p = use_point == "1" && boostable;
    let item_num: i64 = item_id.parse().unwrap_or(-1);

    let has_item = if boostable && item_num != -1 {
        let it_id = item_num as u64;
        crate::state::my_coach(st)
            .get("items")
            .and_then(|v| v.as_array())
            .map(|items| {
                items.iter().any(|i| {
                    i.get("itemid").and_then(|v| v.as_u64()) == Some(it_id)
                        && i.get("remains").and_then(|v| v.as_u64()).unwrap_or(0) > 0
                })
            })
            .unwrap_or(false)
    } else {
        false
    };

    if boostable && item_num != -1 && !has_item {
        return json!({ "result": false, "reason": "expired item" });
    }

    let coach_ep = crate::state::my_coach(st)
        .get("ep")
        .and_then(|v| v.as_i64())
        .unwrap_or(crate::state::START_EP);
    let coach_es = crate::state::my_coach(st)
        .get("es")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if coach_ep < cost as i64 {
        return json!({ "result": false, "reason": "Invalid target ep price" });
    }
    if use_p && coach_es < need as i64 {
        return json!({ "result": false, "reason": "NOT_ENOUGH_ES" });
    }

    let boost_val = if has_item {
        boost_item_val(item_num as u64)
    } else {
        0.0
    };
    let ev_num: i64 = event_id.parse().unwrap_or(-1);
    let rate = enhance25_rate(tg, mg, use_p, boost_val, ev_num == 1001);

    crate::state::add_ep(st, -(cost as i64));
    if use_p {
        crate::state::add_es(st, -(need as i64));
    }

    if has_item {
        let it_id = item_num as u64;
        let coach = crate::state::my_coach_mut(st);
        if let Some(items) = coach.get_mut("items").and_then(|v| v.as_array_mut()) {
            if let Some(it) = items
                .iter_mut()
                .find(|i| i.get("itemid").and_then(|v| v.as_u64()) == Some(it_id))
            {
                let cur = it.get("remains").and_then(|v| v.as_u64()).unwrap_or(1);
                it["remains"] = json!(cur.saturating_sub(1));
            }
            items.retain(|i| i.get("remains").and_then(|v| v.as_u64()).unwrap_or(0) > 0);
        }
    }

    let mut rng = rand::thread_rng();
    let success = rng.gen::<f64>() < rate;

    let after = if success {
        next_g
    } else if tg >= ENHANCE_DROP_FROM {
        tg - 1
    } else {
        tg
    };

    let team = crate::state::my_team_mut(st);
    if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
        if let Some(pa) = players.iter_mut().find(|p| crate::state::iid(p) == target_id) {
            pa["grade"] = json!(after);
        }
    }
    merge_into(team, target_id, material_id);

    let grant = if success { 0 } else { need };
    crate::state::add_es(st, grant as i64);

    if let Some(obj) = st.as_object_mut() {
        obj.remove("enhance_restore");
    }
    let restore_price = (ES_RESTORE * need) as u64;
    if after < tg {
        st["enhance_restore"] = json!({
            "id": target_id,
            "grade": tg,
            "price": restore_price
        });
    }

    let attrib_plus = (GRADE_BOOST[after] as i32) - (GRADE_BOOST[tg] as i32);
    let rprice = crate::state::card_price(spid, after);

    json!({
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
        "resources": [
            {
                "uri": "/coach/me",
                "body": coach_me
            }
        ]
    })
}

pub fn handle_enhance25_restore(st: &mut Value, _slot: usize, coach_me: Value) -> Value {
    let (restore_id, restore_grade, restore_price) = {
        let r = match st.get("enhance_restore") {
            Some(r) if !r.is_null() => r,
            _ => return json!({ "result": false, "reason": "INVALID_PARAM" }),
        };
        let id = match r.get("id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => return json!({ "result": false, "reason": "INVALID_PARAM" }),
        };
        let grade = r.get("grade").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
        let price = r.get("price").and_then(|v| v.as_u64()).unwrap_or(0);
        (id, grade, price)
    };

    let team = crate::state::my_team(st);
    let p_exists = team
        .get("players")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|p| crate::state::iid(p) == restore_id))
        .unwrap_or(false);
    if !p_exists {
        return json!({ "result": false, "reason": "INVALID_PARAM" });
    }

    let coach_es = crate::state::my_coach(st)
        .get("es")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if coach_es < restore_price as i64 {
        return json!({ "result": false, "reason": "NOT_ENOUGH_ES" });
    }

    crate::state::add_es(st, -(restore_price as i64));

    let mut before = 0;
    let mut spid = 0;
    let team_mut = crate::state::my_team_mut(st);
    if let Some(players) = team_mut.get_mut("players").and_then(|v| v.as_array_mut()) {
        if let Some(p) = players.iter_mut().find(|p| crate::state::iid(p) == restore_id) {
            before = crate::state::grade_of(p);
            p["grade"] = json!(restore_grade);
            spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        }
    }

    if let Some(obj) = st.as_object_mut() {
        obj.remove("enhance_restore");
    }

    let attrib_plus = (GRADE_BOOST[restore_grade] as i32) - (GRADE_BOOST[before] as i32);
    let rprice = crate::state::card_price(spid, restore_grade);

    json!({
        "result": true,
        "success": false,
        "grade": restore_grade,
        "rgrade": restore_grade,
        "rprice": rprice,
        "attribPlus": attrib_plus,
        "grantes": 0,
        "usepoint": {
            "currency": "ES",
            "price": restore_price
        },
        "getitems": [],
        "resources": [
            {
                "uri": "/coach/me",
                "body": coach_me
            }
        ]
    })
}

// ---------------------------------------------------------------------------
// Axum Direct Handlers & Router
// ---------------------------------------------------------------------------

pub async fn axum_pe2_staticvalues() -> impl IntoResponse {
    Json(pe2_staticvalues())
}

pub async fn axum_events() -> impl IntoResponse {
    Json(json!({
        "events": [enhance_event()]
    }))
}

pub async fn axum_enhance_preview(
    Path((target, material)): Path<(String, String)>,
) -> impl IntoResponse {
    let _lock = crate::state::STATE_MUTEX.lock().unwrap();
    let st = crate::state::load_state();
    Json(handle_enhance_preview(&st, &target, &material))
}

pub async fn axum_enhance(
    Path((target, material)): Path<(String, String)>,
) -> impl IntoResponse {
    let _lock = crate::state::STATE_MUTEX.lock().unwrap();
    let mut st = crate::state::load_state();
    let slot = crate::state::default_slot(&st);
    let coach_me = crate::http::routes::build_coach_me(&mut st, slot);
    let mut resp = handle_enhance(&mut st, slot, &target, &material, coach_me);
    if resp.get("result").and_then(|v| v.as_bool()) == Some(true) {
        let _ = crate::state::save_state(&st);
        let fresh_coach_me = crate::http::routes::build_coach_me(&mut st, slot);
        resp["resources"] = json!([
            {
                "uri": "/coach/me",
                "body": fresh_coach_me
            }
        ]);
    }
    Json(resp)
}

pub async fn axum_enhance25_preview(
    Path((target, material, use_point, item_id, event_id)): Path<(String, String, String, String, String)>,
) -> impl IntoResponse {
    let _lock = crate::state::STATE_MUTEX.lock().unwrap();
    let st = crate::state::load_state();
    Json(handle_enhance25_preview(&st, &target, &material, &use_point, &item_id, &event_id))
}

pub async fn axum_enhance25(
    Path((target, material, use_point, item_id, event_id)): Path<(String, String, String, String, String)>,
) -> impl IntoResponse {
    let _lock = crate::state::STATE_MUTEX.lock().unwrap();
    let mut st = crate::state::load_state();
    let slot = crate::state::default_slot(&st);
    let coach_me = crate::http::routes::build_coach_me(&mut st, slot);
    let mut resp = handle_enhance25(&mut st, slot, &target, &material, &use_point, &item_id, &event_id, coach_me);
    if resp.get("result").and_then(|v| v.as_bool()) == Some(true) {
        let _ = crate::state::save_state(&st);
        let fresh_coach_me = crate::http::routes::build_coach_me(&mut st, slot);
        resp["resources"] = json!([
            {
                "uri": "/coach/me",
                "body": fresh_coach_me
            }
        ]);
    }
    Json(resp)
}

pub async fn axum_enhance25_restore() -> impl IntoResponse {
    let _lock = crate::state::STATE_MUTEX.lock().unwrap();
    let mut st = crate::state::load_state();
    let slot = crate::state::default_slot(&st);
    let coach_me = crate::http::routes::build_coach_me(&mut st, slot);
    let mut resp = handle_enhance25_restore(&mut st, slot, coach_me);
    if resp.get("result").and_then(|v| v.as_bool()) == Some(true) {
        let _ = crate::state::save_state(&st);
        let fresh_coach_me = crate::http::routes::build_coach_me(&mut st, slot);
        resp["resources"] = json!([
            {
                "uri": "/coach/me",
                "body": fresh_coach_me
            }
        ]);
    }
    Json(resp)
}

pub fn router() -> Router {
    Router::new()
        .route(
            "/player/{target}/enhance/{material}/preview",
            get(axum_enhance_preview),
        )
        .route(
            "/player/{target}/enhance/{material}",
            put(axum_enhance),
        )
        .route(
            "/player/{target}/enhance2.5/{material}/preview/{use_point}/{item_id}/{event_id}",
            get(axum_enhance25_preview),
        )
        .route(
            "/player/{target}/enhance2.5/{material}/{use_point}/{item_id}/{event_id}",
            put(axum_enhance25),
        )
        .route(
            "/player/enhance2.5/restore",
            put(axum_enhance25_restore),
        )
        .route("/static/db/PE2STATICVALUES", get(axum_pe2_staticvalues))
        .route("/events", get(axum_events))
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
        assert_eq!(enhance_es(6), 70); // 10 * (6 + 1)
        assert_eq!(ES_RESTORE * enhance_es(6), 210);
    }

    #[test]
    fn test_boost_items_values() {
        assert_eq!(boost_item_val(1000052), 0.15); // Gold
        assert_eq!(boost_item_val(1000053), 0.10); // Silver
        assert_eq!(boost_item_val(1000054), 0.05); // Bronze
        assert_eq!(boost_item_val(999), 0.0);
    }

    #[test]
    fn test_pe2_staticvalues_structure() {
        let val = pe2_staticvalues();
        assert_eq!(val["bonus"]["use"], 1000);
        assert_eq!(val["simplify_step"]["use"], 0);
        assert_eq!(val["effect_price"]["0"], 10_000_000_000u64);
        assert_eq!(val["effect_price"]["1"], 1_000_000_000u64);
        assert_eq!(val["effect_price"]["2"], 100_000_000u64);
        assert_eq!(val["effect_price"]["3"], 10_000_000u64);
        assert_eq!(val["effect_price"]["4"], 1_000_000u64);
    }
}
