use crate::state;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOCKERROOM_ITEMS: OnceLock<Value> = OnceLock::new();

pub fn get_lockerroom_items() -> &'static Value {
    LOCKERROOM_ITEMS.get_or_init(|| {
        let candidate_paths = [
            PathBuf::from("responses/static_db_LOCKERROOMITEM.json"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("responses/static_db_LOCKERROOMITEM.json"),
        ];
        for p in &candidate_paths {
            if p.is_file() {
                if let Ok(content) = fs::read_to_string(p) {
                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        return val;
                    }
                }
            }
        }
        json!({})
    })
}

pub fn handle_lockerroom_itemset(
    pid: &str,
    body: &[u8],
    st: &mut Value,
    slot: usize,
) -> Option<Value> {
    let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
    let itemset = req.get("itemset").and_then(|v| v.as_object());
    let items = get_lockerroom_items();

    let team = state::my_team_mut(st);
    let players = team.get_mut("players").and_then(|p| p.as_array_mut())?;
    let p = players
        .iter_mut()
        .find(|x| state::iid(x) == pid)?;

    if let Some(set) = itemset {
        let mut lr = serde_json::Map::new();
        for (k, v) in set {
            if items.get(k).is_some() && v.is_object() {
                lr.insert(k.clone(), v.clone());
            }
        }
        if !lr.is_empty() {
            p["lr"] = Value::Object(lr);
        } else if let Some(p_obj) = p.as_object_mut() {
            p_obj.remove("lr");
        }
    } else if let Some(p_obj) = p.as_object_mut() {
        p_obj.remove("lr");
    }

    let _ = state::save_state(st);
    let coach_me_val = crate::http::routes::build_coach_me(st, slot);
    Some(json!({
        "result": true,
        "resources": [
            {
                "uri": "/coach/me",
                "body": coach_me_val
            }
        ]
    }))
}

pub fn handle_lockerroom_jersey(
    pid: &str,
    body: &[u8],
    st: &mut Value,
    slot: usize,
) -> Option<Value> {
    let req: Value = serde_json::from_slice(body).unwrap_or(json!({}));
    let desired = req.get("desired").and_then(|v| v.as_u64()).unwrap_or(0);
    if !(1..=99).contains(&desired) {
        return Some(json!({"result": false}));
    }

    let team = state::my_team_mut(st);
    let players = team.get_mut("players").and_then(|p| p.as_array_mut())?;

    let p_idx = players.iter().position(|x| state::iid(x) == pid)?;
    let old_jersey = players[p_idx].get("jersey").and_then(|v| v.as_u64()).unwrap_or(0);

    // Swap if another player already had this desired number
    if let Some(giver_idx) = players.iter().position(|x| {
        state::iid(x) != pid && x.get("jersey").and_then(|v| v.as_u64()) == Some(desired)
    }) {
        players[giver_idx]["jersey"] = json!(old_jersey);
    }

    players[p_idx]["jersey"] = json!(desired);

    let _ = state::save_state(st);
    let coach_me_val = crate::http::routes::build_coach_me(st, slot);
    Some(json!({
        "result": true,
        "resources": [
            {
                "uri": "/coach/me",
                "body": coach_me_val
            }
        ]
    }))
}
