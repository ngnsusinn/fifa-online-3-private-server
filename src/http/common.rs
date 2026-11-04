use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub const GRADE_BOOST: [u32; 11] = [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24];
pub const GRADE_PRICE: [f64; 11] = [
    0.0, 1.0, 2.65, 7.1, 19.6, 53.0, 162.0, 420.0, 1090.0, 2680.0, 6290.0,
];

pub const MAX_PLAYERS: usize = 200;
pub const FIRST_TEAM_MAX: usize = 36;
pub const POS_SUB: u8 = 18;

pub fn no_boost() -> Value {
    json!({
        "igr": 0.2,
        "vip": 0.3,
        "event": 0,
        "item": 0,
        "discountlimit": 0
    })
}

pub static NO_BOOST: std::sync::LazyLock<Value> = std::sync::LazyLock::new(no_boost);

pub fn sig3(x: u64) -> u64 {
    crate::http::pricing::sig3(x as f64).max(0) as u64
}

pub fn price_formula(ovr: u32) -> u64 {
    crate::http::pricing::price_formula(ovr).max(0) as u64
}

pub fn card_ovr(spid: u32) -> u32 {
    crate::state::card_ovr(spid as u64)
}

pub fn card_price(spid: u32, grade: u8) -> u64 {
    crate::state::card_price(spid as u64, grade as usize)
}

pub fn player_ovr(spid: u32, grade: u8) -> u32 {
    let g = (grade as usize).min(10);
    card_ovr(spid) + GRADE_BOOST[g]
}

pub fn new_iid() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 12] = rng.gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn now_iso() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string()
}

pub fn today_ms() -> u64 {
    let now = Utc::now().timestamp();
    ((now / 86400) * 86400 * 1000) as u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelInfo {
    pub level: u32,
    pub exp: u32,
    pub to: u32,
}

impl Default for LevelInfo {
    fn default() -> Self {
        Self {
            level: 1,
            exp: 0,
            to: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    #[serde(default = "new_iid")]
    pub _id: String,
    pub spid: u32,
    #[serde(default = "default_expandme")]
    pub expandme: String,
    #[serde(default)]
    pub grade: u8,
    #[serde(default = "default_jersey")]
    pub jersey: u8,
    #[serde(default)]
    pub jerseyname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team2: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levelinfo: Option<LevelInfo>,
}

fn default_expandme() -> String {
    "player".to_string()
}

fn default_jersey() -> u8 {
    12
}

impl Player {
    pub fn iid(&self) -> &str {
        &self._id
    }

    pub fn grade_of(&self) -> u8 {
        self.grade.min(10)
    }

    pub fn is_cadangan(&self) -> bool {
        self.team2.unwrap_or(false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SquadPosition {
    pub pos: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub spid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Squad {
    #[serde(default)]
    pub positions: Vec<SquadPosition>,
    #[serde(default)]
    pub kickers: Vec<String>,
    #[serde(default)]
    pub captain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formation: Option<u32>,
}

impl Squad {
    pub fn in_squad(&self, player_id: &str) -> bool {
        self.positions
            .iter()
            .any(|s| s.id.as_deref() == Some(player_id) || s.spid.to_string() == player_id)
    }

    pub fn is_starter(&self, player_id: &str) -> bool {
        self.positions.iter().any(|s| {
            s.pos < POS_SUB
                && (s.id.as_deref() == Some(player_id) || s.spid.to_string() == player_id)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub teamid: u32,
    #[serde(default)]
    pub players: Vec<Player>,
    #[serde(default)]
    pub squad: Squad,
}

impl Team {
    pub fn first_team_count(&self) -> usize {
        self.players.iter().filter(|p| !p.is_cadangan()).count()
    }

    pub fn in_squad(&self, p: &Player) -> bool {
        self.squad.in_squad(&p._id)
    }

    pub fn is_starter(&self, p: &Player) -> bool {
        self.squad.is_starter(&p._id)
    }

    pub fn remove_player(&mut self, player_id: &str) -> Option<Player> {
        if let Some(pos) = self.players.iter().position(|p| p._id == player_id) {
            let removed = self.players.remove(pos);
            self.squad.positions.retain(|s| {
                s.id.as_deref() != Some(player_id) && s.spid.to_string() != player_id
            });
            for k in &mut self.squad.kickers {
                if k == player_id {
                    *k = String::new();
                }
            }
            if self.squad.captain == player_id {
                self.squad.captain = self
                    .squad
                    .positions
                    .first()
                    .and_then(|s| s.id.clone())
                    .unwrap_or_default();
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn add_player(&mut self, spid: u32, grade: u8) -> Player {
        let mut used_jerseys: Vec<u8> = self.players.iter().map(|p| p.jersey).collect();
        used_jerseys.sort_unstable();
        let jersey = (12..100)
            .find(|n| !used_jerseys.contains(n))
            .unwrap_or(99);

        let is_cadangan = self.first_team_count() >= FIRST_TEAM_MAX;
        let p = Player {
            _id: new_iid(),
            spid,
            expandme: "player".to_string(),
            grade,
            jersey,
            jerseyname: String::new(),
            team2: if is_cadangan { Some(true) } else { None },
            levelinfo: Some(LevelInfo::default()),
        };
        self.players.push(p.clone());
        p
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachItem {
    pub _id: String,
    pub itemid: u32,
    pub remains: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staticdata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coach {
    pub _id: String,
    pub slot: usize,
    pub name: String,
    pub teamid: u32,
    #[serde(default = "default_ep")]
    pub ep: u64,
    #[serde(default)]
    pub es: u32,
    #[serde(default)]
    pub items: Vec<CoachItem>,
    #[serde(default)]
    pub scout_used: HashMap<String, f64>,
    #[serde(default)]
    pub levelinfo: LevelInfo,
}

fn default_ep() -> u64 {
    1_000_000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOfferRecord {
    pub _id: String,
    pub status: String,
    pub issuedat: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soldat: Option<String>,
    pub spid: u32,
    pub expandme: String,
    pub grade: u8,
    pub price: u64,
    pub playerobjectid: String,
    pub completefee: BaseFee,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseFee {
    pub base: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeState {
    pub players: Vec<String>,
    pub props: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhanceRestore {
    pub id: String,
    pub grade: u8,
    pub price: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CupPending {
    pub id: String,
    pub cupid: String,
    pub home: u32,
    pub away: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameState {
    pub coaches: Vec<Coach>,
    pub defaultcoachid: String,
    pub teams: HashMap<String, Team>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub league2: Option<Value>,
    #[serde(default)]
    pub cups: HashMap<String, Value>,
    #[serde(default)]
    pub offers: Vec<MarketOfferRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade: Option<TradeState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhance_restore: Option<EnhanceRestore>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cup_pending: Option<CupPending>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_data: Option<Value>,
}

impl GameState {
    pub fn default_slot(&self) -> usize {
        self.coaches
            .iter()
            .find(|c| c._id == self.defaultcoachid)
            .map(|c| c.slot)
            .unwrap_or(0)
    }

    pub fn my_coach(&self) -> Option<&Coach> {
        let slot = self.default_slot();
        self.coaches.iter().find(|c| c.slot == slot)
    }

    pub fn my_coach_mut(&mut self) -> Option<&mut Coach> {
        let slot = self.default_slot();
        self.coaches.iter_mut().find(|c| c.slot == slot)
    }

    pub fn my_teamid(&self) -> u32 {
        self.my_coach().map(|c| c.teamid).unwrap_or(241)
    }

    pub fn my_team(&self) -> Option<&Team> {
        let slot = self.default_slot();
        self.teams.get(&slot.to_string())
    }

    pub fn my_team_mut(&mut self) -> Option<&mut Team> {
        let slot = self.default_slot();
        self.teams.get_mut(&slot.to_string())
    }

    pub fn add_ep(&mut self, delta: i64) {
        if let Some(c) = self.my_coach_mut() {
            if delta < 0 {
                c.ep = c.ep.saturating_sub((-delta) as u64);
            } else {
                c.ep = c.ep.saturating_add(delta as u64);
            }
        }
    }

    pub fn add_es(&mut self, delta: i32) {
        if let Some(c) = self.my_coach_mut() {
            if delta < 0 {
                c.es = c.es.saturating_sub((-delta) as u32);
            } else {
                c.es = c.es.saturating_add(delta as u32);
            }
        }
    }

    pub fn coach_me_body(&self, slot: usize) -> Value {
        let coach = self.coaches.iter().find(|c| c.slot == slot);
        let ep = coach.map(|c| c.ep).unwrap_or(1_000_000);
        let es = coach.map(|c| c.es).unwrap_or(0);
        let name = coach.map(|c| c.name.clone()).unwrap_or_else(|| "Coach".into());
        let cid = coach.map(|c| c._id.clone()).unwrap_or_else(new_iid);
        let teamid = coach.map(|c| c.teamid).unwrap_or(241);
        let team = self.teams.get(&slot.to_string());

        let scouters: Vec<Value> = vec![
            json!({
                "key": "default_B01",
                "scoutid": "default_B01",
                "scoutername": "STR_SCOUTER_KR_DEFAULT_B01_NAME",
                "scouterdesc": "STR_SCOUTER_KR_DEFAULT_B01_DESC",
                "lock": false,
                "quest": { "require_value": 100000 }
            }),
            json!({
                "key": "igr",
                "scoutid": "igr",
                "scoutername": "STR_SCOUTER_KR_IGR_A01_NAME",
                "scouterdesc": "STR_SCOUTER_KR_IGR_A01_DESC",
                "lock": false,
                "quest": { "require_grade": 2, "require_grade_count": 2 }
            }),
            json!({
                "key": "vip",
                "scoutid": "vip",
                "scoutername": "STR_SCOUTER_KR_VIP_A01_NAME",
                "scouterdesc": "STR_SCOUTER_KR_VIP_A01_DESC",
                "lock": false,
                "quest": { "require_season": "88", "require_season_count": 2 }
            }),
            json!({
                "key": "event",
                "scoutid": "event",
                "scoutername": "STR_SCOUTER_KR_EVENT_A01_NAME",
                "scouterdesc": "STR_SCOUTER_KR_EVENT_A01_DESC",
                "lock": false,
                "quest": { "require_season": "93,30,27,71,51,61,89,48,31,33", "require_season_count": 2 }
            }),
            json!({
                "key": "vvip",
                "scoutid": "vvip",
                "scoutername": "STR_SCOUTER_KR_VVIP_A01_NAME",
                "scouterdesc": "STR_SCOUTER_KR_VVIP_A01_DESC",
                "lock": false,
                "quest": { "require_value": 1_000_000_000 }
            }),
        ];

        let mut cup_entries = Vec::new();
        for (cupid, cval) in &self.cups {
            let metaid = cval.get("metaid").and_then(|m| m.as_u64()).unwrap_or(1);
            cup_entries.push(json!({
                "metaid": metaid,
                "cupid": cupid
            }));
        }

        json!({
            "_id": cid,
            "id": cid,
            "slot": slot,
            "name": name,
            "teamid": teamid,
            "levelinfo": coach.map(|c| &c.levelinfo).cloned().unwrap_or_default(),
            "money": {
                "ep": ep,
                "es": es,
                "cash": 0,
                "mp": 0
            },
            "igr": true,
            "vipDetail": 2,
            "vipExpireAt": 2051222400000u64,
            "scouters": scouters,
            "cups": cup_entries,
            "team": team,
            "teamsquadlist": {
                "preset": [team.map(|t| &t.squad)]
            }
        })
    }

    pub fn coach_me_resources(&self, slot: usize) -> Vec<Value> {
        vec![json!({
            "uri": "/coach/me",
            "body": self.coach_me_body(slot)
        })]
    }
}

pub type SharedState = Arc<RwLock<GameState>>;
