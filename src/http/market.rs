use std::path::Path;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::state;

pub const SALE_FEE: f64 = 0.10;

pub fn no_boost() -> Value {
    json!({
        "igr": 0.2,
        "vip": 0.3,
        "event": 0,
        "item": 0,
        "discountlimit": 0,
        "vipDetail": 0
    })
}

pub static NO_BOOST: std::sync::LazyLock<Value> = std::sync::LazyLock::new(no_boost);

pub fn on_market(spid: u64) -> bool {
    // Season 0 engine cards (e.g. 158023 / spid < 1_000_000 that are not licensed/special) are not on market
    if spid == 158023 {
        return false;
    }
    if Path::new(state::FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(state::FIFADB_PATH) {
            let res: Result<i32, _> = conn.query_row(
                "SELECT m.transferable FROM players p JOIN permid m ON m.spid = p.spid WHERE p.spid = ?1",
                [spid],
                |row| row.get(0),
            );
            if let Ok(transferable) = res {
                return transferable == 1;
            }
        }
    }
    // Cards >= 1_000_000 or general non-engine cards
    spid >= 1_000_000 || (spid > 1000 && spid != 158023)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOffer {
    pub spid: u64,
    pub expandme: String,
    pub grade: usize,
    pub price: u64,
    pub count: u32,
}

pub fn market_offer(spid: u64, grade: usize) -> MarketOffer {
    MarketOffer {
        spid,
        expandme: "player".to_string(),
        grade,
        price: state::card_price(spid, grade),
        count: 1,
    }
}

pub fn sale_net(price: u64, fee: u64) -> u64 {
    let igr_discount = (0.2 * fee as f64) as u64;
    let vip_discount = (0.3 * fee as f64) as u64;
    price - fee + igr_discount + vip_discount
}

pub fn offer_list(st: &Value) -> Value {
    let offers = st.get("offers").cloned().unwrap_or_else(|| json!([]));
    json!({
        "offerexpireoffset": 86400000,
        "boost": no_boost(),
        "offers": offers
    })
}

pub use get_market_top_50 as get_top_50_spids;

pub fn get_market_top_50(pos: Option<&str>) -> Vec<u64> {
    if Path::new(state::FIFADB_PATH).exists() {
        if let Ok(conn) = Connection::open(state::FIFADB_PATH) {
            let pos_upper = pos.map(|s| s.to_uppercase());
            let where_clause = match pos_upper.as_deref() {
                Some("GK") => "AND p.preferredposition1 BETWEEN 0 AND 0",
                Some("DF") => "AND p.preferredposition1 BETWEEN 1 AND 8",
                Some("MF") => "AND p.preferredposition1 BETWEEN 9 AND 19",
                Some("FW") => "AND p.preferredposition1 BETWEEN 20 AND 27",
                _ => "",
            };
            let sql = format!(
                "SELECT p.spid FROM players p JOIN permid m ON m.spid = p.spid WHERE m.transferable = 1 AND p.spid >= 1000000 {} ORDER BY p.overallrating DESC, p.spid ASC LIMIT 50",
                where_clause
            );
            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(rows) = stmt.query_map([], |row| row.get::<_, u64>(0)) {
                    let spids: Vec<u64> = rows.flatten().collect();
                    if !spids.is_empty() {
                        return spids;
                    }
                }
            }
        }
    }

    // Fallback if DB not available or empty
    let base_spids: Vec<u64> = match pos.map(|s| s.to_uppercase()).as_deref() {
        Some("GK") => (100100..100170).collect(),
        Some("DF") => (100200..100270).collect(),
        Some("MF") => (100300..100370).collect(),
        Some("FW") => (100400..100470).collect(),
        _ => (1000000..1000080).collect(),
    };

    let mut cards: Vec<(u64, u64)> = base_spids
        .into_iter()
        .filter(|&s| on_market(s))
        .map(|s| (s, state::card_price(s, 1)))
        .collect();

    cards.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    cards.into_iter().take(50).map(|(s, _)| s).collect()
}

pub fn ep_transfer_top(pos: Option<&str>) -> Value {
    let spids = get_market_top_50(pos);
    let offers: Vec<MarketOffer> = spids.into_iter().map(|s| market_offer(s, 1)).collect();
    json!(offers)
}

pub fn ep_transfer_search(spid: u64) -> Value {
    if !on_market(spid) {
        return json!([]);
    }

    let offers = vec![
        market_offer(spid, 1),
        market_offer(spid, 2),
        market_offer(spid, 3),
    ];
    json!(offers)
}

pub fn ep_transfer_price(spid: u64, grade: usize) -> Value {
    let p = state::card_price(spid, grade);
    json!({
        "lower": p * 8 / 10,
        "upper": p,
        "default": p,
        "lowest": p,
        "registrationFee": 0
    })
}

pub fn ep_my_boostfactor() -> Value {
    no_boost()
}

pub fn ep_my_offer_list(st: &Value) -> Value {
    offer_list(st)
}

pub fn purchase(st: &mut Value, spid: u64, grade: usize) -> Result<(), &'static str> {
    if !on_market(spid) {
        return Err("NOT_EXIST_OFFER");
    }

    let team = state::my_team(st);
    let player_count = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    if player_count >= state::MAX_PLAYERS {
        return Err("Exceed plyer limit");
    }

    let cost = state::card_price(spid, grade);
    let coach = state::my_coach(st);
    let ep = coach.get("ep").and_then(|v| v.as_i64()).unwrap_or(0);

    if ep < cost as i64 {
        return Err("NOT_ENOUGH_EP");
    }

    // Deduct EP
    state::add_ep(st, -(cost as i64));

    // Add player to team
    let team = state::my_team_mut(st);
    state::add_player(team, spid, grade as u32);

    let first_team_count = team
        .get("players")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter(|p| !p.get("team2").and_then(|v| v.as_bool()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    if first_team_count > 36 {
        if let Some(players) = team.get_mut("players").and_then(|v| v.as_array_mut()) {
            if let Some(last_p) = players.last_mut() {
                last_p["team2"] = json!(true);
            }
        }
    }

    Ok(())
}

pub fn create_offer(st: &mut Value, pid: &str, price: u64) -> Result<(), &'static str> {
    if pid.is_empty() {
        return Err("TRANSFERMARKET_SELECT_SELLABLE_PLAYER");
    }

    let (spid, grade) = {
        let team = state::my_team(st);
        let players = team
            .get("players")
            .and_then(|v| v.as_array())
            .ok_or("TRANSFERMARKET_SELECT_SELLABLE_PLAYER")?;

        let p = players
            .iter()
            .find(|p| state::iid(p) == pid)
            .ok_or("TRANSFERMARKET_SELECT_SELLABLE_PLAYER")?;

        if state::is_starter(team, p) {
            return Err("TRANSFERMARKET_SELECT_SELLABLE_PLAYER");
        }

        let spid = p.get("spid").and_then(|v| v.as_u64()).unwrap_or(0);
        let grade = state::grade_of(p);
        let max_price = state::card_price(spid, grade);

        if price == 0 || price > max_price {
            return Err("TRANSFERMARKET_LOWERLIMIT");
        }

        (spid, grade)
    };

    let team = state::my_team_mut(st);
    state::remove_player(team, pid);

    let fee = (price as f64 * SALE_FEE) as u64;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string();

    let offer = json!({
        "_id": state::new_iid(),
        "status": "sold",
        "issuedat": now.clone(),
        "soldat": now,
        "spid": spid,
        "expandme": "player",
        "grade": grade,
        "price": price,
        "playerobjectid": pid,
        "completefee": {
            "base": fee
        }
    });

    if st.get("offers").is_none() {
        st["offers"] = json!([]);
    }
    if let Some(arr) = st.get_mut("offers").and_then(|v| v.as_array_mut()) {
        arr.push(offer);
    }

    Ok(())
}

pub fn collect_offer(st: &mut Value, offerid: &str) -> Result<u64, &'static str> {
    let offers = match st.get("offers").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return Err("NOT_EXIST_OFFER"),
    };

    let done: Vec<Value> = offers
        .into_iter()
        .filter(|o| {
            let status = o.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let id = o.get("_id").and_then(|v| v.as_str()).unwrap_or("");
            status == "sold" && (offerid == "all" || id == offerid)
        })
        .collect();

    if done.is_empty() {
        return Err("NOT_EXIST_OFFER");
    }

    let mut total_ep = 0u64;
    for o in &done {
        let price = o.get("price").and_then(|v| v.as_u64()).unwrap_or(0);
        let fee = o
            .get("completefee")
            .and_then(|cf| cf.get("base"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        total_ep += sale_net(price, fee);
    }

    state::add_ep(st, total_ep as i64);

    let done_ids: Vec<String> = done
        .iter()
        .filter_map(|o| o.get("_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .collect();

    if let Some(arr) = st.get_mut("offers").and_then(|v| v.as_array_mut()) {
        arr.retain(|o| {
            let id = o.get("_id").and_then(|v| v.as_str()).unwrap_or("");
            !done_ids.contains(&id.to_string())
        });
    }

    Ok(total_ep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sale_net_with_discounts() {
        let price = 100_000u64;
        let fee = 10_000u64; // 10%
        // net = price - fee + 20%*fee + 30%*fee = 100000 - 10000 + 2000 + 3000 = 95000
        assert_eq!(sale_net(price, fee), 95000);
    }

    #[test]
    fn test_market_offer_generation() {
        let o = market_offer(1001, 1);
        assert_eq!(o.spid, 1001);
        assert_eq!(o.grade, 1);
        assert_eq!(o.count, 1);
        assert!(o.price >= 1000);
    }

    #[test]
    fn test_on_market_filtering() {
        assert!(!on_market(158023)); // Season 0 engine card
        assert!(on_market(1000000));
        assert!(on_market(1001));
    }

    #[test]
    fn test_market_top_50_count() {
        let top = ep_transfer_top(None);
        let arr = top.as_array().unwrap();
        assert_eq!(arr.len(), 50);
        let first = &arr[0];
        assert!(first.get("spid").is_some());
        assert_eq!(first["expandme"], "player");
        assert_eq!(first["grade"], 1);
        assert_eq!(first["count"], 1);
        assert!(first["price"].as_u64().unwrap() > 0);
    }

    #[test]
    fn test_market_search_results() {
        let res = ep_transfer_search(88207995);
        let arr = res.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["grade"], 1);
        assert_eq!(arr[1]["grade"], 2);
        assert_eq!(arr[2]["grade"], 3);

        let unmarketable = ep_transfer_search(158023);
        assert!(unmarketable.as_array().unwrap().is_empty());
    }

    #[test]
    fn test_market_purchase_and_offer_lifecycle() {
        let mut st = state::default_state();
        let slot = state::default_slot(&st);
        let team = state::get_default_team(1);
        st["teams"] = json!({ slot.to_string(): team });

        let coach = state::my_coach_mut(&mut st);
        coach["ep"] = json!(10_000_000);

        // Buy a card
        let spid = 88207995u64;
        let grade = 1usize;
        let price = state::card_price(spid, grade);
        assert!(price > 0);

        let initial_count = state::my_team(&st)["players"].as_array().unwrap().len();
        let res = purchase(&mut st, spid, grade);
        assert!(res.is_ok());

        let new_count = state::my_team(&st)["players"].as_array().unwrap().len();
        assert_eq!(new_count, initial_count + 1);

        let new_ep = state::my_coach(&st)["ep"].as_i64().unwrap();
        assert_eq!(new_ep, 10_000_000 - price as i64);

        // Find the newly purchased player ID (reserve)
        let players = state::my_team(&st)["players"].as_array().unwrap().clone();
        let bought_p = players.iter().find(|p| p["spid"] == spid).unwrap();
        let bought_id = state::iid(bought_p);

        // Try to sell at price = 0 -> error
        let err_price_zero = create_offer(&mut st, &bought_id, 0);
        assert_eq!(err_price_zero, Err("TRANSFERMARKET_LOWERLIMIT"));

        // Try to sell above max price -> error
        let err_price_high = create_offer(&mut st, &bought_id, price + 1000000);
        assert_eq!(err_price_high, Err("TRANSFERMARKET_LOWERLIMIT"));

        // Sell at valid price
        let sell_res = create_offer(&mut st, &bought_id, price);
        assert!(sell_res.is_ok());

        // Player is removed from team
        let after_sell_count = state::my_team(&st)["players"].as_array().unwrap().len();
        assert_eq!(after_sell_count, initial_count);

        // Check offers list
        let offers_val = offer_list(&st);
        let offers = offers_val["offers"].as_array().unwrap();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0]["status"], "sold");
        assert_eq!(offers[0]["spid"], spid);
        assert_eq!(offers[0]["price"], price);

        let offer_id = offers[0]["_id"].as_str().unwrap().to_string();

        // Collect EP for this offer
        let cur_ep_before_collect = state::my_coach(&st)["ep"].as_i64().unwrap();
        let fee = (price as f64 * SALE_FEE) as u64;
        let expected_net = sale_net(price, fee);

        let col_res = collect_offer(&mut st, &offer_id);
        assert_eq!(col_res, Ok(expected_net));

        let cur_ep_after_collect = state::my_coach(&st)["ep"].as_i64().unwrap();
        assert_eq!(cur_ep_after_collect, cur_ep_before_collect + expected_net as i64);

        // Offer is removed from offers list
        let after_col_offers = offer_list(&st)["offers"].as_array().unwrap().clone();
        assert!(after_col_offers.is_empty());
    }
}
