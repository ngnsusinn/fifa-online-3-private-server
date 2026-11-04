use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::http::constants::{
    default_formation_442, Formation, POS_SUB, START_EP,
};

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum StateError {
    Io(io::Error),
    Json(serde_json::Error),
    LockError,
    CoachNotFound(u32),
    TeamNotFound(String),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StateError::Io(e) => write!(f, "I/O error: {}", e),
            StateError::Json(e) => write!(f, "JSON error: {}", e),
            StateError::LockError => write!(f, "State lock poisoned"),
            StateError::CoachNotFound(s) => write!(f, "Coach at slot {} not found", s),
            StateError::TeamNotFound(s) => write!(f, "Team {} not found", s),
        }
    }
}

impl std::error::Error for StateError {}

impl From<io::Error> for StateError {
    fn from(e: io::Error) -> Self {
        StateError::Io(e)
    }
}

impl From<serde_json::Error> for StateError {
    fn from(e: serde_json::Error) -> Self {
        StateError::Json(e)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a 24-character hexadecimal Mongo-style ID
pub fn new_iid() -> String {
    let mut bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut hex = String::with_capacity(24);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

/// Coach Mongo-style _id from slot
pub fn coach_id(slot: u32) -> String {
    format!("{:024x}", slot + 1)
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LevelInfo {
    pub level: u32,
    pub exp: u64,
    pub to: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamValue {
    pub val: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildupTactics {
    pub buildupspeed: u32,
    pub passing: u32,
    pub positioning: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChanceCreationTactics {
    pub passing: u32,
    pub crossing: u32,
    pub shooting: u32,
    pub positioning: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefenseTactics {
    pub mentality: u32,
    pub aggression: u32,
    pub teamwidth: u32,
    pub defenderline: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tactics {
    pub id: u32,
    pub name: String,
    pub teammentality: u32,
    pub bus: BuildupTactics,
    pub cc: ChanceCreationTactics,
    #[serde(rename = "def")]
    pub def_: DefenseTactics,
}

pub fn default_tactic(i: u32) -> Tactics {
    Tactics {
        id: i,
        name: "".to_string(),
        teammentality: 3,
        bus: BuildupTactics {
            buildupspeed: 50,
            passing: 50,
            positioning: 0,
        },
        cc: ChanceCreationTactics {
            passing: 50,
            crossing: 50,
            shooting: 50,
            positioning: 0,
        },
        def_: DefenseTactics {
            mentality: 50,
            aggression: 50,
            teamwidth: 50,
            defenderline: 0,
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SquadPosition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub spid: u32,
    pub pos: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Squad {
    pub positions: Vec<SquadPosition>,
    #[serde(default)]
    pub formation: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captain: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kickers: Option<Vec<u32>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Player {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,
    pub spid: u32,
    #[serde(default)]
    pub grade: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levelinfo: Option<LevelInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jersey: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jerseyname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team2: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expandme: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl Player {
    pub fn iid(&self) -> String {
        self._id
            .as_ref()
            .or(self.id.as_ref())
            .cloned()
            .unwrap_or_else(|| self.spid.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OwnedTeam {
    pub teamid: u32,
    pub squad: Squad,
    pub players: Vec<Player>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formations: Option<Vec<Formation>>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub _id: String,
    pub itemid: u32,
    pub remains: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expireable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expired: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staticdata: Option<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagItem {
    pub itemid: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagPlayer {
    pub spid: u32,
    #[serde(default)]
    pub grade: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagEntry {
    pub _id: String,
    pub title: String,
    pub msg: String,
    #[serde(default)]
    pub items: Vec<BagItem>,
    #[serde(default)]
    pub players: Vec<BagPlayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recvdate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyRewardState {
    pub last: String,
    pub streak: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coach {
    pub _id: String,
    pub slot: u32,
    pub name: String,
    #[serde(default = "default_active_state")]
    pub state: String,
    pub teamid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teamvalue: Option<TeamValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levelinfo: Option<LevelInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deleteat: Option<f64>,
    #[serde(default)]
    pub items: Vec<InventoryItem>,
    #[serde(default)]
    pub bag: Vec<BagEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily: Option<DailyRewardState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ep: Option<i64>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn default_active_state() -> String {
    "active".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GameState {
    #[serde(default)]
    pub coaches: Vec<Coach>,
    #[serde(default)]
    pub defaultcoachid: String,
    #[serde(default)]
    pub teams: HashMap<String, OwnedTeam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tactics: Option<HashMap<String, HashMap<String, Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub league2: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cups: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_state: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offers: Option<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ---------------------------------------------------------------------------
// State Manager with Thread-Safe Locking & Persistence
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct StateManager {
    path: PathBuf,
    state: Arc<RwLock<GameState>>,
}

impl StateManager {
    /// Initialize with path to state.json. Attempts to load existing file,
    /// or creates default empty state if not found.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, StateError> {
        let path = path.into();
        let state = if path.exists() {
            Self::load_from_disk(&path)?
        } else {
            GameState::default()
        };

        Ok(Self {
            path,
            state: Arc::new(RwLock::new(state)),
        })
    }

    /// Default state manager pointing to `gamedata/state.json`
    pub fn default_manager() -> Result<Self, StateError> {
        let path = std::env::var("FO3_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("gamedata/state.json"));
        Self::new(path)
    }

    /// Read raw state from disk
    pub fn load_from_disk(path: &Path) -> Result<GameState, StateError> {
        let content = fs::read_to_string(path)?;
        let state: GameState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Atomic write to disk using temporary file in same directory
    pub fn save_to_disk(path: &Path, state: &GameState) -> Result<(), StateError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = path.with_extension(format!("tmp.{}", new_iid()));
        {
            let mut file = File::create(&tmp_path)?;
            let json_bytes = serde_json::to_vec_pretty(state)?;
            file.write_all(&json_bytes)?;
            file.sync_all()?;
        }

        // Atomic rename
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Execute a read-only closure over the state
    pub fn read<F, R>(&self, f: F) -> Result<R, StateError>
    where
        F: FnOnce(&GameState) -> R,
    {
        let guard = self.state.read().map_err(|_| StateError::LockError)?;
        Ok(f(&guard))
    }

    /// Execute a modifying closure over the state and save to disk atomically
    pub fn write<F, R>(&self, f: F) -> Result<R, StateError>
    where
        F: FnOnce(&mut GameState) -> Result<R, StateError>,
    {
        let mut guard = self.state.write().map_err(|_| StateError::LockError)?;
        let result = f(&mut guard)?;
        Self::save_to_disk(&self.path, &guard)?;
        Ok(result)
    }

    /// Get current active coach slot
    pub fn default_slot(&self) -> Result<u32, StateError> {
        self.read(|st| {
            st.coaches
                .iter()
                .find(|c| c._id == st.defaultcoachid && c.state == "active")
                .map(|c| c.slot)
                .or_else(|| {
                    st.coaches
                        .iter()
                        .find(|c| c.state == "active")
                        .map(|c| c.slot)
                })
                .unwrap_or(0)
        })
    }

    /// Add EP to a coach at slot
    pub fn add_ep(&self, slot: u32, amount: i64) -> Result<i64, StateError> {
        self.write(|st| {
            let coach = st
                .coaches
                .iter_mut()
                .find(|c| c.slot == slot)
                .ok_or(StateError::CoachNotFound(slot))?;

            let current = coach.ep.unwrap_or(START_EP);
            let next = (current + amount).max(0);
            coach.ep = Some(next);
            Ok(next)
        })
    }

    /// Add an item stack to coach's inventory
    pub fn inventory_add(&self, slot: u32, itemid: u32, count: u32) -> Result<u32, StateError> {
        self.write(|st| {
            let coach = st
                .coaches
                .iter_mut()
                .find(|c| c.slot == slot)
                .ok_or(StateError::CoachNotFound(slot))?;

            if let Some(it) = coach.items.iter_mut().find(|i| i.itemid == itemid) {
                it.remains += count;
                it.count = Some(it.remains);
                return Ok(it.remains);
            }

            let new_item = InventoryItem {
                _id: new_iid(),
                itemid,
                remains: count,
                count: Some(count),
                expireable: Some(true),
                expired: Some(false),
                staticdata: None,
                extra: HashMap::new(),
            };
            coach.items.push(new_item);
            Ok(count)
        })
    }

    /// Add an entry to coach's Bag
    pub fn bag_add(
        &self,
        slot: u32,
        title: &str,
        items: Vec<BagItem>,
        players: Vec<BagPlayer>,
        msg: &str,
    ) -> Result<BagEntry, StateError> {
        self.write(|st| {
            let coach = st
                .coaches
                .iter_mut()
                .find(|c| c.slot == slot)
                .ok_or(StateError::CoachNotFound(slot))?;

            let entry = BagEntry {
                _id: new_iid(),
                title: title.to_string(),
                msg: msg.to_string(),
                items,
                players,
                recvdate: Some(chrono::Utc::now().to_rfc3339()),
            };

            coach.bag.push(entry.clone());
            Ok(entry)
        })
    }

    /// Register/create a new coach at slot
    pub fn create_coach(&self, slot: u32, name: &str, teamid: u32) -> Result<(), StateError> {
        self.write(|st| {
            st.coaches.retain(|c| c.slot != slot);
            let id = coach_id(slot);
            let coach = Coach {
                _id: id.clone(),
                slot,
                name: name.to_string(),
                state: "active".to_string(),
                teamid,
                teamvalue: Some(TeamValue { val: 0 }),
                levelinfo: Some(LevelInfo {
                    level: 1,
                    exp: 0,
                    to: 500,
                }),
                summary: Some(json!({})),
                appat: Some(chrono::Utc::now().to_rfc3339()),
                deleteat: None,
                items: Vec::new(),
                bag: Vec::new(),
                daily: Some(DailyRewardState::default()),
                ep: Some(START_EP),
                extra: HashMap::new(),
            };
            if st.defaultcoachid.is_empty() {
                st.defaultcoachid = id;
            }
            st.coaches.push(coach);
            Ok(())
        })
    }
}

/// Fills squad to ensure 11 starters + 7 bench players (18 entries total)
pub fn fill_squad(sq: &mut Squad, players: &[Player]) {
    let mut taken: std::collections::HashSet<u32> = sq
        .positions
        .iter()
        .map(|s| s.spid % 1_000_000)
        .collect();

    let mut spare = Vec::new();
    let mut sorted_players: Vec<&Player> = players
        .iter()
        .filter(|p| p.state.as_deref() == Some("first"))
        .collect();
    sorted_players.sort_by_key(|p| -(p.price.unwrap_or(0) as i64));

    for p in sorted_players {
        let base_id = p.spid % 1_000_000;
        if !taken.contains(&base_id) {
            taken.insert(base_id);
            spare.push(SquadPosition {
                id: Some(p.iid()),
                spid: p.spid,
                pos: POS_SUB,
                starting: None,
            });
        }
    }

    let default_442 = default_formation_442();
    let shape = &default_442.positions;

    let used_positions: std::collections::HashSet<u32> = sq
        .positions
        .iter()
        .filter(|s| s.pos < POS_SUB)
        .map(|s| s.pos)
        .collect();

    let holes: Vec<u32> = shape
        .iter()
        .copied()
        .filter(|pos| !used_positions.contains(pos))
        .collect();

    let mut subs: Vec<SquadPosition> = sq
        .positions
        .iter()
        .filter(|s| s.pos == POS_SUB)
        .cloned()
        .collect();
    subs.extend(spare);

    for (hole, mut candidate) in holes.into_iter().zip(subs.into_iter()) {
        candidate.pos = hole;
        candidate.starting = None;
        if !sq.positions.iter().any(|p| p.spid == candidate.spid) {
            sq.positions.push(candidate);
        }
    }

    // Top up subs up to 7
    let current_subs = sq.positions.iter().filter(|s| s.pos == POS_SUB).count();
    let needed_subs = 7usize.saturating_sub(current_subs);
    let mut added = 0;
    for p in players.iter() {
        if added >= needed_subs || sq.positions.len() >= 18 {
            break;
        }
        if !sq.positions.iter().any(|s| s.spid == p.spid) {
            sq.positions.push(SquadPosition {
                id: Some(p.iid()),
                spid: p.spid,
                pos: POS_SUB,
                starting: None,
            });
            added += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_iid_length() {
        let id = new_iid();
        assert_eq!(id.len(), 24);
    }

    #[test]
    fn test_coach_id_format() {
        assert_eq!(coach_id(0), "000000000000000000000001");
        assert_eq!(coach_id(1), "000000000000000000000002");
    }

    #[test]
    fn test_state_manager_save_and_load() {
        let tmp_dir = std::env::temp_dir().join(format!("fo3_test_{}", new_iid()));
        let state_file = tmp_dir.join("state.json");

        let manager = StateManager::new(&state_file).unwrap();
        manager.create_coach(0, "ManagerTest", 241).unwrap();

        let coach = manager.read(|st| st.coaches[0].clone()).unwrap();
        assert_eq!(coach.name, "ManagerTest");
        assert_eq!(coach.teamid, 241);

        // Modify EP
        manager.add_ep(0, 500_000).unwrap();
        let ep = manager.read(|st| st.coaches[0].ep.unwrap()).unwrap();
        assert_eq!(ep, START_EP + 500_000);

        // Add inventory item
        manager.inventory_add(0, 28001, 2).unwrap();
        let items = manager.read(|st| st.coaches[0].items.clone()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].remains, 2);

        // Add bag item
        manager
            .bag_add(
                0,
                "Welcome Bag",
                vec![BagItem {
                    itemid: 501,
                    count: 3,
                }],
                vec![],
                "Test message",
            )
            .unwrap();

        let bag = manager.read(|st| st.coaches[0].bag.clone()).unwrap();
        assert_eq!(bag.len(), 1);
        assert_eq!(bag[0].title, "Welcome Bag");

        // Reload from disk to verify atomic save
        let reloaded = StateManager::new(&state_file).unwrap();
        let reloaded_name = reloaded.read(|st| st.coaches[0].name.clone()).unwrap();
        assert_eq!(reloaded_name, "ManagerTest");

        // Cleanup
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
