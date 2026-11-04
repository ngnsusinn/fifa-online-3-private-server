use super::common::{
    card_price, new_iid, now_iso, no_boost, BaseFee, GameState, MarketOfferRecord, SharedState,
    MAX_PLAYERS,
};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const SALE_FEE: f64 = 0.10;

pub fn on_market(spid: u32) -> bool {
    // Season 0 engine cards (e.g. 158023 / spid < 1_000_000 that are not licensed/special) are not on market
    if spid == 158023 {
        return false;
    }
    // Cards >= 1_000_000 are market cards
    spid >= 1_000_000 || spid > 1000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOffer {
    pub spid: u32,
    pub expandme: String,
    pub grade: u8,
    pub price: u64,
    pub count: u32,
}

pub fn market_offer(spid: u32, grade: u8) -> MarketOffer {
    MarketOffer {
        spid,
        expandme: "player".to_string(),
        grade,
        price: card_price(spid, grade),
        count: 1,
    }
}

pub fn sale_net(price: u64, fee: u64) -> u64 {
    let igr_discount = (0.2 * fee as f64) as u64;
    let vip_discount = (0.3 * fee as f64) as u64;
    price - fee + igr_discount + vip_discount
}

pub fn offer_list(st: &GameState) -> Value {
    json!({
        "offerexpireoffset": 86400000,
        "boost": no_boost(),
        "offers": st.offers
    })
}

pub fn offer_resources(st: &GameState) -> Vec<Value> {
    let slot = st.default_slot();
    let mut res = vec![json!({
        "uri": "/transfer/my/offer/list",
        "body": offer_list(st)
    })];
    res.extend(st.coach_me_resources(slot));
    res
}

pub fn generate_market_cards(pos: Option<&str>) -> Vec<u32> {
    let base_spids: Vec<u32> = match pos.map(|s| s.to_uppercase()).as_deref() {
        Some("GK") => (100100..100170).collect(),
        Some("DF") => (100200..100270).collect(),
        Some("MF") => (100300..100370).collect(),
        Some("FW") => (100400..100470).collect(),
        _ => (1000000..1000080).collect(),
    };

    let mut cards: Vec<(u32, u64)> = base_spids
        .into_iter()
        .filter(|&s| on_market(s))
        .map(|s| (s, card_price(s, 1)))
        .collect();

    cards.sort_by(|a, b| b.1.cmp(&a.1));
    cards.into_iter().take(50).map(|(s, _)| s).collect()
}

// ---------------------------------------------------------------------------
// HTTP Handlers
// ---------------------------------------------------------------------------

pub async fn ep_transfer_top(pos: Option<Path<String>>) -> impl IntoResponse {
    let position = pos.map(|p| p.0);
    let spids = generate_market_cards(position.as_deref());
    let offers: Vec<MarketOffer> = spids.into_iter().map(|s| market_offer(s, 1)).collect();
    Json(offers)
}

pub async fn ep_transfer_search(Path(spid_str): Path<String>) -> impl IntoResponse {
    let spid: u32 = match spid_str.parse() {
        Ok(s) => s,
        Err(_) => return Json(vec![]),
    };

    if !on_market(spid) {
        return Json(vec![]);
    }

    let offers = vec![
        market_offer(spid, 1),
        market_offer(spid, 2),
        market_offer(spid, 3),
    ];
    Json(offers)
}

pub async fn ep_transfer_price(
    Path((spid_str, grade_str)): Path<(String, String)>,
) -> impl IntoResponse {
    let spid: u32 = spid_str.parse().unwrap_or(1000000);
    let grade: u8 = grade_str.parse().unwrap_or(1);
    let p = card_price(spid, grade);

    Json(json!({
        "lower": p * 8 / 10,
        "upper": p,
        "default": p,
        "lowest": p,
        "registrationFee": 0
    }))
}

pub async fn ep_transfer_purchase(
    State(state): State<SharedState>,
    Path((spid_str, grade_str, _price_str)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let spid: u32 = match spid_str.parse() {
        Ok(s) => s,
        Err(_) => return Json(json!({ "result": false, "reason": "NOT_EXIST_OFFER" })),
    };
    let grade: u8 = grade_str.parse().unwrap_or(1);

    if !on_market(spid) {
        return Json(json!({ "result": false, "reason": "NOT_EXIST_OFFER" }));
    }

    let mut st = state.write().await;
    let slot = st.default_slot();

    let cost = card_price(spid, grade);
    let coach_ep = st.my_coach().map(|c| c.ep).unwrap_or(0);
    let player_count = st.my_team().map(|t| t.players.len()).unwrap_or(0);

    if player_count >= MAX_PLAYERS {
        return Json(json!({ "result": false, "reason": "Exceed plyer limit" }));
    }
    if coach_ep < cost {
        return Json(json!({ "result": false, "reason": "NOT_ENOUGH_EP" }));
    }

    st.add_ep(-(cost as i64));
    if let Some(team) = st.my_team_mut() {
        team.add_player(spid, grade);
    }

    let resources = st.coach_me_resources(slot);
    Json(json!({
        "result": true,
        "resources": resources
    }))
}

#[derive(Debug, Deserialize)]
pub struct OfferReq {
    pub playerobjectid: Option<String>,
    pub price: Option<u64>,
}

pub async fn ep_transfer_offer(
    State(state): State<SharedState>,
    Json(req): Json<OfferReq>,
) -> impl IntoResponse {
    let pid = match req.playerobjectid {
        Some(id) => id,
        None => return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELECT_SELLABLE_PLAYER" })),
    };
    let price = req.price.unwrap_or(0);

    let mut st = state.write().await;

    let (spid, grade) = {
        let team = match st.my_team() {
            Some(t) => t,
            None => return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELECT_SELLABLE_PLAYER" })),
        };

        let player = match team.players.iter().find(|p| p._id == pid) {
            Some(p) => p,
            None => return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELECT_SELLABLE_PLAYER" })),
        };

        if team.is_starter(player) {
            return Json(json!({ "result": false, "reason": "TRANSFERMARKET_SELECT_SELLABLE_PLAYER" }));
        }

        let max_price = card_price(player.spid, player.grade_of());
        if price == 0 || price > max_price {
            return Json(json!({ "result": false, "reason": "TRANSFERMARKET_LOWERLIMIT" }));
        }

        (player.spid, player.grade_of())
    };

    if let Some(team) = st.my_team_mut() {
        team.remove_player(&pid);
    }

    let fee = (price as f64 * SALE_FEE) as u64;
    let now = now_iso();

    st.offers.push(MarketOfferRecord {
        _id: new_iid(),
        status: "sold".to_string(),
        issuedat: now.clone(),
        soldat: Some(now),
        spid,
        expandme: "player".to_string(),
        grade,
        price,
        playerobjectid: pid,
        completefee: BaseFee { base: fee },
    });

    let res = offer_resources(&st);
    Json(json!({
        "result": true,
        "resources": res
    }))
}

pub async fn ep_offer_collect(
    State(state): State<SharedState>,
    Path(offerid): Path<String>,
) -> impl IntoResponse {
    let mut st = state.write().await;

    let to_collect: Vec<MarketOfferRecord> = st
        .offers
        .iter()
        .filter(|o| o.status == "sold" && (offerid == "all" || o._id == offerid))
        .cloned()
        .collect();

    if to_collect.is_empty() {
        return Json(json!({ "result": false, "reason": "NOT_EXIST_OFFER" }));
    }

    let mut total_ep = 0u64;
    for o in &to_collect {
        total_ep += sale_net(o.price, o.completefee.base);
    }

    st.add_ep(total_ep as i64);

    let collected_ids: Vec<String> = to_collect.into_iter().map(|o| o._id).collect();
    st.offers.retain(|o| !collected_ids.contains(&o._id));

    let res = offer_resources(&st);
    Json(json!({
        "result": true,
        "ep": total_ep,
        "resources": res
    }))
}

pub async fn ep_my_offer_list(State(state): State<SharedState>) -> impl IntoResponse {
    let st = state.read().await;
    Json(offer_list(&st))
}

pub async fn ep_my_boostfactor() -> impl IntoResponse {
    Json(no_boost())
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/transfer/top/50", get(|| ep_transfer_top(None)))
        .route(
            "/transfer/top/50/position/{pos}",
            get(|Path(pos): Path<String>| ep_transfer_top(Some(Path(pos)))),
        )
        .route("/transfer/search/{spid}", get(ep_transfer_search))
        .route(
            "/transfer/purchase/{spid}/{grade}/{price}/purchase",
            post(ep_transfer_purchase),
        )
        .route("/transfer/price/{spid}/{grade}", get(ep_transfer_price))
        .route("/transfer/offer", post(ep_transfer_offer))
        .route("/transfer/my/offer/list", get(ep_my_offer_list))
        .route("/transfer/my/boostfactor", get(ep_my_boostfactor))
        .route(
            "/transfer/my/offer/completed/{offerid}",
            delete(ep_offer_collect),
        )
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
}
