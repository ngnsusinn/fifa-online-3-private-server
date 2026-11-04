use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Position Constants
// ---------------------------------------------------------------------------

pub const POS_NAMES: [&str; 28] = [
    "GK", "SW", "RWB", "RB", "RCB", "CB", "LCB", "LB", "LWB", "RDM", "CDM", "LDM",
    "RM", "RCM", "CM", "LCM", "LM", "RAM", "CAM", "LAM", "RF", "CF", "LF", "RW",
    "RS", "ST", "LS", "LW",
];

pub const POS_SUB: u32 = 28; // 0..27 pitch positions, 28 SUB, 29+ RESERVE

pub const POS_ROLE: [&str; 28] = [
    "GK", "SW", "WB", "FB", "CB", "CB", "CB", "FB", "WB", "DM", "DM", "DM",
    "WM", "CM", "CM", "CM", "WM", "AM", "AM", "AM", "CF", "CF", "CF", "W",
    "ST", "ST", "ST", "W",
];

pub const ROLE_WEIGHTS: [(&str, &[(&str, u32)]); 12] = [
    ("GK", &[
        ("gkdiving", 21), ("gkhandling", 21), ("gkkicking", 5),
        ("gkpositioning", 21), ("gkreflexes", 21), ("reactions", 11),
    ]),
    ("SW", &[
        ("aggression", 8), ("ballcontrol", 5), ("headingaccuracy", 10),
        ("jumping", 4), ("marking", 15), ("reactions", 5),
        ("shortpassing", 5), ("slidingtackle", 15), ("standingtackle", 15),
        ("strength", 10), ("interceptions", 8),
    ]),
    ("WB", &[
        ("acceleration", 4), ("ballcontrol", 8), ("crossing", 12),
        ("dribbling", 4), ("marking", 7), ("reactions", 8),
        ("shortpassing", 10), ("slidingtackle", 11), ("sprintspeed", 6),
        ("stamina", 10), ("standingtackle", 8), ("interceptions", 12),
    ]),
    ("FB", &[
        ("acceleration", 5), ("ballcontrol", 7), ("crossing", 9),
        ("headingaccuracy", 4), ("marking", 8), ("reactions", 8),
        ("shortpassing", 7), ("slidingtackle", 14), ("sprintspeed", 7),
        ("stamina", 8), ("standingtackle", 11), ("interceptions", 12),
    ]),
    ("CB", &[
        ("aggression", 7), ("ballcontrol", 4), ("headingaccuracy", 10),
        ("jumping", 3), ("marking", 14), ("reactions", 5),
        ("shortpassing", 5), ("slidingtackle", 17), ("sprintspeed", 2),
        ("standingtackle", 10), ("strength", 10), ("interceptions", 13),
    ]),
    ("DM", &[
        ("aggression", 5), ("ballcontrol", 10), ("longpassing", 10),
        ("marking", 9), ("reactions", 7), ("shortpassing", 14),
        ("slidingtackle", 5), ("stamina", 6), ("standingtackle", 12),
        ("strength", 4), ("interceptions", 14), ("vision", 4),
    ]),
    ("WM", &[
        ("acceleration", 7), ("ballcontrol", 13), ("crossing", 10),
        ("dribbling", 15), ("finishing", 6), ("longpassing", 5),
        ("positioning", 8), ("reactions", 7), ("shortpassing", 11),
        ("sprintspeed", 6), ("stamina", 5), ("vision", 7),
    ]),
    ("CM", &[
        ("ballcontrol", 14), ("dribbling", 7), ("finishing", 2),
        ("longpassing", 13), ("longshots", 4), ("positioning", 6),
        ("reactions", 8), ("shortpassing", 17), ("stamina", 6),
        ("standingtackle", 5), ("interceptions", 5), ("vision", 13),
    ]),
    ("AM", &[
        ("acceleration", 4), ("agility", 3), ("ballcontrol", 15),
        ("dribbling", 13), ("finishing", 7), ("longpassing", 4),
        ("longshots", 5), ("positioning", 9), ("reactions", 7),
        ("shortpassing", 16), ("sprintspeed", 3), ("vision", 14),
    ]),
    ("CF", &[
        ("acceleration", 5), ("ballcontrol", 15), ("dribbling", 14),
        ("finishing", 11), ("headingaccuracy", 2), ("longshots", 4),
        ("positioning", 13), ("reactions", 9), ("shortpassing", 9),
        ("shotpower", 5), ("sprintspeed", 5), ("vision", 8),
    ]),
    ("W", &[
        ("acceleration", 7), ("agility", 3), ("ballcontrol", 14),
        ("crossing", 9), ("dribbling", 16), ("finishing", 10),
        ("longshots", 4), ("positioning", 9), ("reactions", 7),
        ("shortpassing", 9), ("sprintspeed", 6), ("vision", 6),
    ]),
    ("ST", &[
        ("acceleration", 4), ("ballcontrol", 10), ("dribbling", 7),
        ("finishing", 18), ("headingaccuracy", 10), ("longshots", 3),
        ("positioning", 13), ("reactions", 8), ("shortpassing", 5),
        ("shotpower", 10), ("sprintspeed", 5), ("strength", 5),
        ("volleys", 2),
    ]),
];

// ---------------------------------------------------------------------------
// Level & Experience Curves
// ---------------------------------------------------------------------------

pub const PLAYER_MAX_LEVEL: u32 = 23;
pub const COACH_MAX_LEVEL: u32 = 100;

pub const EXP_WIN: (u32, u32) = (300, 150);
pub const EXP_DRAW: (u32, u32) = (200, 100);
pub const EXP_LOSS: (u32, u32) = (100, 60);

/// 24 entries for Player level EXP requirement (levels 0..23)
pub const EXP_PLAYER: [u32; 24] = [
    0, 0, 80, 160, 260, 380, 520, 680, 860, 1060,
    1280, 1520, 1780, 2060, 2360, 2680, 3020, 3380, 3760, 4160,
    4580, 5020, 5480, 5960,
];

/// Linear formula from http_config.py: 100 * level
pub fn player_need(level: u32) -> u32 {
    100 * level
}

/// Coach exp need formula from http_config.py: 500 + 100 * (level - 1)
pub fn coach_need(level: u32) -> u64 {
    500 + 100 * (level.max(1) as u64 - 1)
}

/// EXPCOACH formula: 200 * lvl * lvl
pub fn coach_total_exp(lvl: u32) -> u64 {
    200 * (lvl as u64) * (lvl as u64)
}

/// Cumulative exp needed to reach level using coach_need
pub fn coach_cumulative_exp(lvl: u32) -> u64 {
    (1..=lvl).map(coach_need).sum()
}

// ---------------------------------------------------------------------------
// Grade Pricing and Enhancements
// ---------------------------------------------------------------------------

/// EP price multiplier of grade relative to +1 (indices 0..10)
pub const GRADE_PRICE: [f64; 11] = [
    0.0, 1.0, 2.65, 7.1, 19.6, 53.0, 162.0, 420.0, 1090.0, 2680.0, 6290.0,
];

/// Relative upgrade boost (+0..+10) compared to +1: [0, 0, 1, 2, 3, 5, 7, 9, 11, 13, 15]
pub const GRADE_BOOST: [u32; 11] = [0, 0, 1, 2, 3, 5, 7, 9, 11, 13, 15];

/// All-attribute boost (+0..+10) from PLAYERGRADE.json: [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24]
pub const GRADE_ALL_ATTRIBUTES: [u32; 11] = [0, 5, 6, 7, 8, 10, 12, 14, 17, 20, 24];

// ---------------------------------------------------------------------------
// Control & Sound Defaults
// ---------------------------------------------------------------------------

pub const SOUND_DEFAULTS: [(&str, u32); 7] = [
    ("Master", 10),
    ("Commentary", 10),
    ("PAVolume", 10),
    ("PlayerCalls", 10),
    ("SFX", 10),
    ("Crowd", 10),
    ("BGM", 10),
];

const WASD: [u32; 8] = [87, 65, 68, 83, 81, 90, 67, 69];
const ARROWS: [u32; 7] = [38, 40, 37, 39, 256, 257, 258];

const KEY_ATT: [&[u32]; 4] = [
    &WASD,
    &WASD,
    &[69, 65, 68, 83, 81, 90, 67, 87],
    &[80, 75, 186, 76, 73, 190, 191, 79],
];

const KEY_DEF: [&[u32]; 8] = [
    &WASD,
    &[87, 65, 83, 81, 68, 90, 67, 69],
    KEY_ATT[2],
    KEY_ATT[3],
    &[87, 65, 68, 32, 81, 90, 67, 69],
    &[87, 65, 83, 32, 68, 90, 67, 69],
    &[69, 65, 68, 32, 81, 90, 67, 87],
    &[80, 75, 186, 32, 73, 190, 191, 79],
];

const KEY_MOV: [&[u32]; 4] = [
    &ARROWS,
    &ARROWS,
    &ARROWS,
    &[87, 83, 65, 68, 256, 257, 258],
];

const JOY: [&[u32]; 3] = [
    &[4, 3, 2, 1, 5, 7, 6, 8],
    &[4, 2, 3, 1, 5, 6, 8, 7],
    &[4, 3, 2, 1, 5, 7, 6, 8],
];

const JOY_MOV: [&[u32]; 3] = [
    &[9, 11, 10],
    &[9, 11, 10],
    &[11, 9, 10],
];

pub fn control_defaults_json() -> Value {
    let mut map = serde_json::Map::new();
    map.insert("Controller".to_string(), json!(0));
    map.insert("Type".to_string(), json!(0));

    for i in 0..8 {
        map.insert(format!("KeyAttType{}", i + 1), json!(KEY_ATT[i % 4]));
        map.insert(format!("KeyDefType{}", i + 1), json!(KEY_DEF[i]));
        map.insert(format!("KeyMovType{}", i + 1), json!(KEY_MOV[i % 4]));
    }

    for i in 0..3 {
        map.insert(format!("JoyAttType{}", i + 1), json!(JOY[i]));
        map.insert(format!("JoyDefType{}", i + 1), json!(JOY[i]));
        map.insert(format!("JoyMovType{}", i + 1), json!(JOY_MOV[i]));
    }

    Value::Object(map)
}

pub fn is_valid_control_option(c: &Value) -> bool {
    let obj = match c.as_object() {
        Some(o) => o,
        None => return false,
    };

    let check_int = |k: &str| obj.get(k).and_then(Value::as_i64).is_some();
    let check_list = |k: &str| {
        obj.get(k)
            .and_then(Value::as_array)
            .map(|arr| !arr.is_empty() && arr.iter().all(|x| x.as_i64().is_some()))
            .unwrap_or(false)
    };

    if !check_int("Controller") || !check_int("Type") {
        return false;
    }

    for i in 1..=8 {
        if !check_list(&format!("KeyAttType{}", i))
            || !check_list(&format!("KeyDefType{}", i))
            || !check_list(&format!("KeyMovType{}", i))
        {
            return false;
        }
    }

    for i in 1..=3 {
        if !check_list(&format!("JoyAttType{}", i))
            || !check_list(&format!("JoyDefType{}", i))
            || !check_list(&format!("JoyMovType{}", i))
        {
            return false;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Formations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormationOffset {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Formation {
    pub formationid: u32,
    pub formationname: String,
    pub positions: Vec<u32>,
    pub offsets: Vec<FormationOffset>,
}

pub fn default_formation_442() -> Formation {
    Formation {
        formationid: 1,
        formationname: "4-4-2".to_string(),
        positions: vec![0, 3, 4, 6, 7, 12, 13, 15, 16, 24, 26],
        offsets: vec![
            FormationOffset { x: 0.5, y: 0.02 },  // GK
            FormationOffset { x: 0.85, y: 0.22 }, // RB
            FormationOffset { x: 0.62, y: 0.16 }, // RCB
            FormationOffset { x: 0.38, y: 0.16 }, // LCB
            FormationOffset { x: 0.15, y: 0.22 }, // LB
            FormationOffset { x: 0.85, y: 0.5 },  // RM
            FormationOffset { x: 0.62, y: 0.46 }, // RCM
            FormationOffset { x: 0.38, y: 0.46 }, // LCM
            FormationOffset { x: 0.15, y: 0.5 },  // LM
            FormationOffset { x: 0.62, y: 0.8 },  // RS
            FormationOffset { x: 0.38, y: 0.8 },  // LS
        ],
    }
}

// ---------------------------------------------------------------------------
// Season and Pack Definitions
// ---------------------------------------------------------------------------

pub const FA_SEASONS: [(&str, &[u32], u32); 39] = [
    ("06", &[6], 45004),
    ("06U", &[63], 7618),
    ("06WC", &[65], 7622),
    ("08", &[8], 739),
    ("08E", &[67], 8157),
    ("09", &[9], 45006),
    ("10", &[10], 45007),
    ("10U", &[69], 7618),
    ("10WC", &[47], 7622),
    ("11", &[11], 45004),
    ("14", &[14], 15031),
    ("14WC", &[77], 880047),
    ("14T", &[53], 45026),
    ("15", &[15], 16001),
    ("16", &[16], 16001),
    ("17", &[88], 45011),
    ("16EC", &[35], 8157),
    ("16TH", &[45], 8545),
    ("WB", &[91], 8334),
    ("WL", &[93], 8194),
    ("UB", &[28], 8336),
    ("UL", &[30], 8335),
    ("EL", &[27], 8194),
    ("LP", &[23], 8329),
    ("CP", &[22], 8549),
    ("CC", &[24], 8547),
    ("17TH", &[44], 8545),
    ("TCO92", &[49], 8546),
    ("MUA", &[55], 8547),
    ("ML", &[71], 8549),
    ("TL", &[51], 8549),
    ("VL", &[61], 8549),
    ("CL", &[89], 8549),
    ("KL02", &[95], 8546),
    ("CEL", &[31, 33], 8335),
    ("U23", &[29], 8545),
    ("KXI", &[97], 8546),
    ("MUL", &[48], 8547),
    ("VS", &[73], 2000006),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFilter {
    pub league: Option<u32>,
    pub ovr: Option<u32>,
    pub pos: Option<String>,
    pub season: Option<u32>,
    pub seasons: Option<Vec<u32>>,
    pub top: Option<usize>,
    pub grade: Option<u32>,
    pub all: bool,
    pub icon: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackDef {
    pub itemid: u32,
    pub name: String,
    pub filter: PackFilter,
}

pub fn get_pack_catalog() -> HashMap<u32, PackDef> {
    let mut packs = HashMap::new();

    let add = |m: &mut HashMap<u32, PackDef>, id: u32, name: &str, f: PackFilter| {
        m.insert(
            id,
            PackDef {
                itemid: id,
                name: name.to_string(),
                filter: f,
            },
        );
    };

    let base_packs: &[(u32, &str, Option<u32>, Option<u32>, Option<&str>, Option<u32>, Option<&[u32]>, Option<usize>, Option<u32>, bool)] = &[
        (28001, "Premier League 65+", Some(13), Some(65), None, None, None, None, None, false),
        (28002, "Bundesliga 65+", Some(19), Some(65), None, None, None, None, None, false),
        (28003, "LaLiga 65+", Some(53), Some(65), None, None, None, None, None, false),
        (28004, "Serie A 65+", Some(31), Some(65), None, None, None, None, None, false),
        (28005, "Ligue 1 65+", Some(16), Some(65), None, None, None, None, None, false),
        (28006, "FW 60+", None, Some(60), Some("FW"), None, None, None, None, false),
        (28007, "MF 60+", None, Some(60), Some("MF"), None, None, None, None, false),
        (28008, "DF 60+", None, Some(60), Some("DF"), None, None, None, None, false),
        (28009, "GK 60+", None, Some(60), Some("GK"), None, None, None, None, false),
        (8223600, "FW 65+", None, Some(65), Some("FW"), None, None, None, None, false),
        (8223601, "MF 65+", None, Some(65), Some("MF"), None, None, None, None, false),
        (8223602, "DF 65+", None, Some(65), Some("DF"), None, None, None, None, false),
        (8223603, "GK 65+", None, Some(65), Some("GK"), None, None, None, None, false),
        (8223508, "FW NEW 70+ 2017", None, Some(70), Some("FW"), Some(88), None, None, None, false),
        (8223509, "MF NEW 70+ 2017", None, Some(70), Some("MF"), Some(88), None, None, None, false),
        (8223510, "DF NEW 70+ 2017", None, Some(70), Some("DF"), Some(88), None, None, None, false),
        (82214068, "World Best 300 2017", None, None, None, Some(88), None, Some(300), None, false),
        (82214180, "World Best 50 2017", None, None, None, Some(88), None, Some(50), None, false),
        (45011, "This Season Top 10", None, None, None, Some(88), None, Some(10), None, false),
        (45012, "This Season Top 20", None, None, None, Some(88), None, Some(20), None, false),
        (45013, "This Season Top 30", None, None, None, Some(88), None, Some(30), None, false),
        (45097, "This Season Top 30 +8", None, None, None, Some(88), None, Some(30), Some(8), false),
        (45004, "06-11 Player Top 10", None, None, None, None, Some(&[6, 7, 8, 9, 10, 11]), Some(10), None, false),
        (45006, "06-11 Player Top 30", None, None, None, None, Some(&[6, 7, 8, 9, 10, 11]), Some(30), None, false),
        (45007, "06-11 Player Top 50", None, None, None, None, Some(&[6, 7, 8, 9, 10, 11]), Some(50), None, false),
        (82214356, "07 Top 200", None, None, None, Some(7), None, Some(200), None, true),
        (739, "'08 Top 3", None, None, None, Some(8), None, Some(3), None, false),
        (15031, "'14 Top 30 +5", None, None, None, Some(14), None, Some(30), Some(5), false),
        (16001, "'15 Top 30 +5", None, None, None, Some(15), None, Some(30), Some(5), false),
        (7622, "World Cup '10 Top 30", None, None, None, Some(47), None, Some(30), None, false),
        (7618, "Europe '10 Top 30 +3", None, None, None, Some(69), None, Some(30), Some(3), false),
        (880047, "World Cup '14 Top 50", None, None, None, Some(77), None, Some(50), None, false),
        (45020, "Special Season '14 Top 30", None, None, None, Some(77), None, Some(30), None, false),
        (45063, "Special Season '14 Top 50 +5", None, None, None, Some(77), None, Some(50), Some(5), false),
        (45026, "Team of the Season '14 Top 20", None, None, None, Some(53), None, Some(20), None, false),
        (45027, "Team of the Season '14 Top 30", None, None, None, Some(53), None, Some(30), None, false),
        (45028, "Team of the Season '14 Top 50", None, None, None, Some(53), None, Some(50), None, false),
        (45130, "Team of the Season '14 All", None, None, None, Some(53), None, None, None, false),
        (8157, "EC Top 10", None, None, None, Some(35), None, Some(10), None, false),
        (82232124, "EC Top 30 +3", None, None, None, Some(35), None, Some(30), Some(3), false),
        (8135, "Best EC 30 +5", None, None, None, Some(35), None, Some(30), Some(5), false),
        (8329, "LP Top 50 +3", None, None, None, Some(23), None, Some(50), Some(3), false),
        (8334, "World Best 5", None, None, None, Some(91), None, Some(5), None, false),
        (8336, "World Best 1", None, None, None, Some(91), None, Some(1), None, false),
        (82231307, "World Best +5", None, None, None, Some(91), None, None, Some(5), false),
        (8194, "World Legend Top 30", None, None, None, Some(93), None, Some(30), None, false),
        (8335, "World Legend Top 3", None, None, None, Some(93), None, Some(3), None, false),
        (82214007, "Top 300", None, None, None, None, None, Some(300), None, false),
        (8223562, "Best 30", None, None, None, None, None, Some(30), None, false),
        (7675, "All Season Top 300 +8", None, Some(86), None, None, None, None, Some(8), true),
    ];

    for &(id, name, league, ovr, pos, season, seasons, top, grade, all) in base_packs.iter() {
        add(
            &mut packs,
            id,
            name,
            PackFilter {
                league,
                ovr,
                pos: pos.map(String::from),
                season,
                seasons: seasons.map(|s| s.to_vec()),
                top,
                grade,
                all,
                icon: None,
            },
        );
    }

    // 39 FA Season Top 200 packs
    for &(label, codes, icon) in FA_SEASONS.iter() {
        let id = 92000000 + codes[0];
        add(
            &mut packs,
            id,
            &format!("{} Top 200", label),
            PackFilter {
                league: None,
                ovr: None,
                pos: None,
                season: None,
                seasons: Some(codes.to_vec()),
                top: Some(200),
                grade: None,
                all: true,
                icon: Some(icon),
            },
        );
    }

    // Semua Pemain
    add(
        &mut packs,
        92000000,
        "Semua Pemain",
        PackFilter {
            league: None,
            ovr: None,
            pos: None,
            season: None,
            seasons: None,
            top: None,
            grade: None,
            all: true,
            icon: Some(7675),
        },
    );

    packs
}

// ---------------------------------------------------------------------------
// General Game Constants
// ---------------------------------------------------------------------------

pub const EP_ITEM: u32 = 1;
pub const ES_ITEM: u32 = 2; // Upgrade points
pub const START_EP: i64 = 100_000_000;
pub const MAX_PLAYERS: usize = 100;
pub const COACH_RECOVERY: f64 = 300.0; // 5 minutes recovery period
pub const WIB: i64 = 7 * 3600; // UTC+7 in seconds
pub const VIP_UNTIL: i64 = 4_102_444_800_000; // timestamp ms: ~Jan 2100

pub const TC_CLUB: u32 = 17;
pub const TC_NATION: u32 = 131;
pub const TC_SEASON: u32 = 50000;

pub const TC_GRADE: [(u32, u32, &str, &str); 11] = [
    (10002, 2, "TEAMCOLOR_ATTRIBUTES_EVOLVE24", "Pemain grade +2 ke atas"),
    (10005, 5, "TEAMCOLOR_ATTRIBUTES_EVOLVE10011", "Pemain grade +5 ke atas"),
    (10008, 8, "TEAMCOLOR_ATTRIBUTES_EVOLVE810", "Pemain grade +8 ke atas"),
    (10103, 3, "TEAMCOLOR_ATTRIBUTES_EVOLVE3", "Pemain grade +3 ke atas"),
    (10104, 4, "TEAMCOLOR_ATTRIBUTES_EVOLVE4", "Pemain grade +4 ke atas"),
    (10105, 5, "TEAMCOLOR_ATTRIBUTES_EVOLVE5", "Pemain grade +5 ke atas"),
    (10106, 6, "TEAMCOLOR_ATTRIBUTES_EVOLVE6", "Pemain grade +6 ke atas"),
    (10107, 7, "TEAMCOLOR_ATTRIBUTES_EVOLVE7", "Pemain grade +7 ke atas"),
    (10108, 8, "TEAMCOLOR_ATTRIBUTES_EVOLVE8", "Pemain grade +8 ke atas"),
    (10109, 9, "TEAMCOLOR_ATTRIBUTES_EVOLVE9", "Pemain grade +9 ke atas"),
    (10110, 10, "TEAMCOLOR_ATTRIBUTES_EVOLVE10", "Pemain grade +10 ke atas"),
];

pub const TC_LEGENDS: [u32; 10] = [93, 30, 27, 71, 51, 61, 89, 48, 55, 49];
pub const TC_YEARS: [u32; 10] = [6, 7, 8, 9, 10, 11, 14, 15, 16, 88];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_defaults_count() {
        let v = control_defaults_json();
        let obj = v.as_object().unwrap();
        assert_eq!(obj.len(), 35);
        assert!(is_valid_control_option(&v));
    }

    #[test]
    fn test_pos_names_and_roles() {
        assert_eq!(POS_NAMES.len(), 28);
        assert_eq!(POS_ROLE.len(), 28);
        assert_eq!(POS_SUB, 28);
    }

    #[test]
    fn test_role_weights_sum() {
        for &(role, weights) in ROLE_WEIGHTS.iter() {
            let total: u32 = weights.iter().map(|(_, w)| w).sum();
            assert_eq!(total, 100, "Role {} weights sum to {}", role, total);
        }
    }

    #[test]
    fn test_exp_curves() {
        assert_eq!(EXP_PLAYER.len(), 24);
        assert_eq!(EXP_PLAYER[0], 0);
        assert_eq!(EXP_PLAYER[1], 0);
        assert_eq!(EXP_PLAYER[2], 80);
        assert_eq!(EXP_PLAYER[3], 160);
        assert_eq!(EXP_PLAYER[4], 260);

        assert_eq!(coach_total_exp(1), 200);
        assert_eq!(coach_total_exp(2), 800);
        assert_eq!(coach_total_exp(10), 20000);
    }

    #[test]
    fn test_grade_constants() {
        assert_eq!(GRADE_BOOST.len(), 11);
        assert_eq!(GRADE_PRICE.len(), 11);
        assert_eq!(GRADE_ALL_ATTRIBUTES.len(), 11);

        assert_eq!(GRADE_BOOST[0], 0);
        assert_eq!(GRADE_BOOST[1], 0);
        assert_eq!(GRADE_BOOST[5], 5);
        assert_eq!(GRADE_BOOST[10], 15);
    }

    #[test]
    fn test_pack_catalog_count() {
        let packs = get_pack_catalog();
        assert_eq!(packs.len(), 90);
    }
}
