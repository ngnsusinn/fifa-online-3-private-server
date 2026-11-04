use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use crate::http::constants::{
    FA_SEASONS, POS_SUB, TC_CLUB, TC_GRADE, TC_NATION, TC_SEASON,
};
use crate::http::db::FifaDb;
use crate::http::state::{Player, SquadPosition};

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamColorBoost {
    #[serde(rename = "type")]
    pub boost_type: String, // e.g. "ovr", "sprintspeed", "stamina"
    pub amount: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>, // "fw", "mf", "df", "gk", or None (all)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamColorBoostGroup {
    pub count_min: u32,
    pub count_max: u32,
    pub boosts: Vec<TeamColorBoost>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamColorDef {
    pub teamcolorid: u32,
    #[serde(rename = "type")]
    pub color_type: String, // "club", "nation", "season", "special", "grade"
    pub teamid: Value,      // -1, or integer id, or string id for specials
    pub name: String,
    pub description: String,
    pub boostgroups: Vec<TeamColorBoostGroup>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveTeamColor {
    pub teamcolorid: u32,
    pub boostindex: usize,
    pub teamid: Value,
}

// ---------------------------------------------------------------------------
// Boost Group Builders
// ---------------------------------------------------------------------------

fn make_ovr_tier(min: u32, max: u32, n: u32) -> TeamColorBoostGroup {
    TeamColorBoostGroup {
        count_min: min,
        count_max: max,
        boosts: vec![TeamColorBoost {
            boost_type: "ovr".to_string(),
            amount: n,
            target: None,
        }],
    }
}

pub fn club_groups() -> Vec<TeamColorBoostGroup> {
    vec![
        make_ovr_tier(4, 5, 1),
        make_ovr_tier(6, 7, 2),
        make_ovr_tier(8, 9, 3),
        make_ovr_tier(10, 11, 4),
    ]
}

pub fn standard_groups() -> Vec<TeamColorBoostGroup> {
    vec![
        make_ovr_tier(5, 6, 1),
        make_ovr_tier(7, 8, 2),
        make_ovr_tier(9, 11, 3),
    ]
}

fn add_boost_to_groups(
    groups: &[TeamColorBoostGroup],
    target: &str,
    stat: &str,
    n: u32,
) -> Vec<TeamColorBoostGroup> {
    groups
        .iter()
        .map(|g| {
            let mut boosts = g.boosts.clone();
            boosts.push(TeamColorBoost {
                boost_type: stat.to_string(),
                amount: n,
                target: if target.is_empty() {
                    None
                } else {
                    Some(target.to_string())
                },
            });
            TeamColorBoostGroup {
                count_min: g.count_min,
                count_max: g.count_max,
                boosts,
            }
        })
        .collect()
}

fn special_groups(
    target: &str,
    stat: &str,
    n: u32,
    min_players: u32,
) -> Vec<TeamColorBoostGroup> {
    let top = if stat == "ovr" { n } else { 3 };
    let base = if min_players < 5 {
        vec![make_ovr_tier(min_players, 11, top)]
    } else {
        vec![
            make_ovr_tier(5, 6, top.saturating_sub(2).max(1)),
            make_ovr_tier(7, 8, top.saturating_sub(1).max(1)),
            make_ovr_tier(9, 11, top),
        ]
    };

    if stat == "ovr" {
        base
    } else {
        add_boost_to_groups(&base, target, stat, n)
    }
}

pub fn tc_tier(groups: &[TeamColorBoostGroup], count: u32) -> Option<usize> {
    groups
        .iter()
        .position(|g| count >= g.count_min && count <= g.count_max)
}

// ---------------------------------------------------------------------------
// Static Definitions for all 188 Team Colors
// ---------------------------------------------------------------------------

pub const TC_NAMED_CLUBS: [(u32, u32, &str, &str, u32); 27] = [
    (1, 243, "fw", "sprintspeed", 2),
    (2, 241, "mf", "agility", 3),
    (3, 9, "fw", "balance", 1),
    (4, 10, "fw", "longpassing", 2),
    (5, 11, "fw", "reactions", 2),
    (6, 1, "mf", "stamina", 2),
    (7, 5, "mf", "dribbling", 1),
    (8, 18, "mf", "acceleration", 2),
    (9, 22, "mf", "positioning", 2),
    (10, 32, "fw", "sprintspeed", 8),
    (11, 21, "mf", "finishing", 2),
    (12, 47, "fw", "positioning", 1),
    (13, 45, "df", "stamina", 2),
    (14, 44, "fw", "dribbling", 1),
    (15, 73, "fw", "ballcontrol", 2),
    (16, 982, "mf", "stamina", 12),
    (43, 240, "", "ovr", 1),
    (44, 52, "", "ovr", 1),
    (45, 236, "", "ballcontrol", 3),
    (46, 69, "", "stamina", 2),
    (47, 110374, "", "positioning", 3),
    (48, 48, "", "dribbling", 2),
    (146, 981, "", "ovr", 1),
    (885, 110955, "", "headingaccuracy", 4),
    (20054, 112172, "", "ovr", 1),
    (20059, 72, "", "ovr", 1),
    (20060, 1477, "", "ovr", 1),
];

pub const TC_NAMED_NATIONS: [(u32, u32, &str, &str, u32); 30] = [
    (101, 45, "mf", "ballcontrol", 2),
    (102, 21, "fw", "positioning", 3),
    (103, 54, "fw", "dribbling", 2),
    (104, 38, "fw", "dribbling", 2),
    (105, 52, "fw", "balance", 2),
    (106, 60, "fw", "agility", 2),
    (107, 56, "mf", "shotpower", 3),
    (108, 27, "mf", "shortpassing", 3),
    (109, 14, "mf", "stamina", 1),
    (110, 7, "mf", "ballcontrol", 2),
    (111, 95, "fw", "jumping", 2),
    (112, 34, "fw", "reactions", 1),
    (113, 18, "fw", "positioning", 2),
    (114, 10, "fw", "headingaccuracy", 2),
    (115, 40, "fw", "ballcontrol", 2),
    (116, 83, "fw", "reactions", 2),
    (117, 108, "mf", "reactions", 2),
    (118, 117, "mf", "shortpassing", 2),
    (119, 167, "mf", "aggression", 2),
    (120, 163, "mf", "shortpassing", 2),
    (121, 195, "fw", "positioning", 2),
    (122, 55, "fw", "sprintspeed", 2),
    (123, 49, "fw", "finishing", 1),
    (124, 50, "fw", "reactions", 2),
    (125, 46, "fw", "shotpower", 2),
    (126, 13, "fw", "headingaccuracy", 2),
    (127, 51, "fw", "jumping", 2),
    (128, 103, "fw", "acceleration", 2),
    (129, 133, "fw", "sprintspeed", 1),
    (130, 12, "fw", "positioning", 2),
];

pub const TC_SEASON_NAMES: [(u32, u32, &str, &str, u32); 12] = [
    (91, 18, "", "ovr", 5),
    (93, 100, "", "ovr", 3),
    (95, 19, "", "ovr", 3),
    (77, 42, "", "reactions", 1),
    (65, 140, "", "ovr", 3),
    (53, 141, "", "ovr", 3),
    (47, 142, "", "ovr", 3),
    (63, 143, "", "ovr", 3),
    (69, 137, "", "ovr", 3),
    (67, 138, "", "ovr", 3),
    (35, 145, "", "ovr", 3),
    (27, 20044, "", "ovr", 3),
];

#[derive(Debug, Clone)]
pub struct SpecialRuleDef {
    pub cid: u32,
    pub desc: &'static str,
    pub target: &'static str,
    pub stat: &'static str,
    pub amount: u32,
    pub best: Option<usize>,
    pub min_players: u32,
}

pub const TC_NAMED_SPECIALS: [SpecialRuleDef; 78] = [
    // 20..39: Pemain 14 WC dari negara ini (20 rules)
    SpecialRuleDef { cid: 20, desc: "Pemain 14 WC dari negara ini", target: "", stat: "ovr", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 21, desc: "Pemain 14 WC dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 22, desc: "Pemain 14 WC dari negara ini", target: "", stat: "ballcontrol", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 23, desc: "Pemain 14 WC dari negara ini", target: "", stat: "positioning", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 24, desc: "Pemain 14 WC dari negara ini", target: "", stat: "dribbling", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 25, desc: "Pemain 14 WC dari negara ini", target: "fw", stat: "longshots", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 26, desc: "Pemain 14 WC dari negara ini", target: "", stat: "strength", amount: 1, best: None, min_players: 5 },
    SpecialRuleDef { cid: 27, desc: "Pemain 14 WC dari negara ini", target: "", stat: "agility", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 28, desc: "Pemain 14 WC dari negara ini", target: "", stat: "longpassing", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 29, desc: "Pemain 14 WC dari negara ini", target: "", stat: "shortpassing", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 30, desc: "Pemain 14 WC dari negara ini", target: "mf", stat: "stamina", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 31, desc: "Pemain 14 WC dari negara ini", target: "", stat: "ballcontrol", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 32, desc: "Pemain 14 WC dari negara ini", target: "", stat: "strength", amount: 1, best: None, min_players: 5 },
    SpecialRuleDef { cid: 33, desc: "Pemain 14 WC dari negara ini", target: "fw", stat: "acceleration", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 34, desc: "Pemain 14 WC dari negara ini", target: "", stat: "positioning", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 35, desc: "Pemain 14 WC dari negara ini", target: "", stat: "headingaccuracy", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 36, desc: "Pemain 14 WC dari negara ini", target: "", stat: "marking", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 37, desc: "Pemain 14 WC dari negara ini", target: "", stat: "reactions", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 38, desc: "Pemain 14 WC dari negara ini", target: "", stat: "reactions", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 39, desc: "Pemain 14 WC dari negara ini", target: "", stat: "shortpassing", amount: 2, best: None, min_players: 5 },
    // 20007..20017: 50 kartu terbaik negara ini (11 rules)
    SpecialRuleDef { cid: 20007, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20008, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20009, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20010, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20011, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20012, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20013, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20014, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20015, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20016, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 20017, desc: "50 kartu terbaik negara ini", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    // 20018..20027: Pemain EC / Copa dari negara ini (10 rules)
    SpecialRuleDef { cid: 20018, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20019, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20020, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20021, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20022, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20023, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20024, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20025, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20026, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20027, desc: "Pemain EC / Copa dari negara ini", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    // 133..136: Pemain season '10 klub ini (4 rules)
    SpecialRuleDef { cid: 133, desc: "Pemain season '10 klub ini", target: "fw", stat: "positioning", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 134, desc: "Pemain season '10 klub ini", target: "fw", stat: "sprintspeed", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 135, desc: "Pemain season '10 klub ini", target: "mf", stat: "aggression", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 136, desc: "Pemain season '10 klub ini", target: "mf", stat: "vision", amount: 5, best: None, min_players: 5 },
    // Standalone special rules
    SpecialRuleDef { cid: 132, desc: "Pemain season spesial (bukan tahunan, bukan legenda)", target: "", stat: "ovr", amount: 2, best: None, min_players: 5 },
    SpecialRuleDef { cid: 139, desc: "Juara liga 2016/17 (season '17)", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 41, desc: "30 kartu terbaik", target: "", stat: "shortpassing", amount: 2, best: Some(30), min_players: 5 },
    SpecialRuleDef { cid: 2014, desc: "Kartu OVR 85 ke atas", target: "", stat: "ovr", amount: 4, best: None, min_players: 5 },
    SpecialRuleDef { cid: 886, desc: "Pemain legenda dari China", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    // Iconic trios & teams
    SpecialRuleDef { cid: 20002, desc: "Messi, Suarez, Neymar", target: "", stat: "ovr", amount: 3, best: None, min_players: 3 },
    SpecialRuleDef { cid: 20003, desc: "Bale, Benzema, Cristiano Ronaldo", target: "", stat: "ovr", amount: 3, best: None, min_players: 3 },
    SpecialRuleDef { cid: 3059, desc: "Messi dan Cristiano Ronaldo", target: "", stat: "ovr", amount: 3, best: None, min_players: 2 },
    SpecialRuleDef { cid: 3060, desc: "Ramos, Pepe, Marcelo, Carvajal, Varane, Navas", target: "", stat: "ovr", amount: 3, best: None, min_players: 6 },
    SpecialRuleDef { cid: 3058, desc: "Leicester City juara 2015/16", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 3064, desc: "Pemain AC Milan dan Inter", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 34024, desc: "Pemain AC Milan (rossoneri)", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20030, desc: "Arsenal 2003/04 tak terkalahkan", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20031, desc: "Shevchenko dan Kaka", target: "", stat: "ovr", amount: 3, best: None, min_players: 2 },
    SpecialRuleDef { cid: 20032, desc: "Terry, Lampard, Drogba, Cech", target: "", stat: "ovr", amount: 3, best: None, min_players: 4 },
    SpecialRuleDef { cid: 20033, desc: "Figo, Ronaldo, Beckham, Raul, Roberto Carlos, Casillas", target: "", stat: "ovr", amount: 3, best: None, min_players: 6 },
    SpecialRuleDef { cid: 20028, desc: "Pemain season '17", target: "", stat: "ovr", amount: 3, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20029, desc: "Pemain season apa saja", target: "", stat: "ovr", amount: 1, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20043, desc: "Pemain season legenda", target: "", stat: "ovr", amount: 4, best: None, min_players: 5 },
    SpecialRuleDef { cid: 40001, desc: "50 kartu terbaik", target: "", stat: "ovr", amount: 3, best: Some(50), min_players: 5 },
    SpecialRuleDef { cid: 40002, desc: "100 kartu terbaik", target: "", stat: "ovr", amount: 3, best: Some(100), min_players: 5 },
    SpecialRuleDef { cid: 40003, desc: "100 kartu terbaik season spesial", target: "", stat: "ovr", amount: 3, best: Some(100), min_players: 5 },
    SpecialRuleDef { cid: 40004, desc: "100 kartu terbaik season '17", target: "", stat: "ovr", amount: 3, best: Some(100), min_players: 5 },
    SpecialRuleDef { cid: 40005, desc: "100 kartu terbaik season legenda", target: "", stat: "ovr", amount: 3, best: Some(100), min_players: 5 },
    // Attribute and Condition Specials
    SpecialRuleDef { cid: 20063, desc: "Pemain Brasil skill 4 bintang ke atas", target: "", stat: "ballcontrol", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20064, desc: "Short pass dan vision 85 ke atas", target: "", stat: "shortpassing", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20065, desc: "Pemain Real Madrid dan Barcelona", target: "", stat: "dribbling", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20066, desc: "Tackle 85 ke atas", target: "", stat: "aggression", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20067, desc: "Work rate menyerang dan bertahan tinggi", target: "", stat: "positioning", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20068, desc: "Bek tinggi 188 cm ke atas", target: "", stat: "strength", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20069, desc: "Lahir 1995 ke atas", target: "", stat: "longshots", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20070, desc: "Heading 85 ke atas", target: "", stat: "strength", amount: 5, best: None, min_players: 5 },
    SpecialRuleDef { cid: 20071, desc: "Reputasi internasional 4 bintang ke atas", target: "", stat: "ovr", amount: 4, best: None, min_players: 5 },
];

/// Builds all 188 team color definitions
pub fn get_all_team_colors() -> Vec<TeamColorDef> {
    let mut colors = Vec::with_capacity(188);

    // 1. Generic Club
    colors.push(TeamColorDef {
        teamcolorid: TC_CLUB,
        color_type: "club".to_string(),
        teamid: json!(-1),
        name: "TEAMCOLOR_ATTRIBUTES_TEAM17".to_string(),
        description: "Pemain dari klub yang sama".to_string(),
        boostgroups: club_groups(),
        grade: None,
    });

    // 2. Generic Nation
    colors.push(TeamColorDef {
        teamcolorid: TC_NATION,
        color_type: "nation".to_string(),
        teamid: json!(-1),
        name: "TEAMCOLOR_ATTRIBUTES_TEAM131".to_string(),
        description: "Pemain dari negara yang sama".to_string(),
        boostgroups: standard_groups(),
        grade: None,
    });

    // 3. 11 Grade Enhancement Colors
    for &(cid, g, name, _) in TC_GRADE.iter() {
        let groups = if cid == 10002 {
            standard_groups()
        } else if cid == 10005 {
            vec![
                make_ovr_tier(5, 6, 2),
                make_ovr_tier(7, 8, 3),
                make_ovr_tier(9, 11, 4),
            ]
        } else if cid == 10008 {
            vec![
                make_ovr_tier(5, 6, 3),
                make_ovr_tier(7, 8, 4),
                make_ovr_tier(9, 11, 5),
            ]
        } else {
            let n = match g {
                3 | 4 => 4,
                5 | 6 | 7 => 5,
                8 | 9 => 6,
                _ => 7,
            };
            vec![make_ovr_tier(11, 11, n)]
        };

        colors.push(TeamColorDef {
            teamcolorid: cid,
            color_type: "grade".to_string(),
            teamid: json!(-1),
            name: name.to_string(),
            description: format!("Pemain grade +{} ke atas", g),
            boostgroups: groups,
            grade: Some(g),
        });
    }

    // 4. 27 Named Clubs
    for &(cid, teamid, target, stat, amount) in TC_NAMED_CLUBS.iter() {
        let groups = add_boost_to_groups(&club_groups(), target, stat, amount);
        colors.push(TeamColorDef {
            teamcolorid: cid,
            color_type: "club".to_string(),
            teamid: json!(teamid),
            name: format!("TEAMCOLOR_ATTRIBUTES_TEAM{}", cid),
            description: "Pemain klub ini (sekarang atau dulu)".to_string(),
            boostgroups: groups,
            grade: None,
        });
    }

    // 5. 30 Named Nations
    for &(cid, nationid, target, stat, amount) in TC_NAMED_NATIONS.iter() {
        let groups = add_boost_to_groups(&standard_groups(), target, stat, amount);
        colors.push(TeamColorDef {
            teamcolorid: cid,
            color_type: "nation".to_string(),
            teamid: json!(nationid),
            name: format!("TEAMCOLOR_ATTRIBUTES_TEAM{}", cid),
            description: "Pemain dari negara ini".to_string(),
            boostgroups: groups,
            grade: None,
        });
    }

    // 6. 40 Season Colors (07 + 39 FA_SEASONS)
    let seasons: Vec<(&str, &[u32])> = std::iter::once(("07", &[7u32][..]))
        .chain(FA_SEASONS.iter().map(|&(l, c, _)| (l, c)))
        .collect();

    for &(label, codes) in seasons.iter() {
        let cid = TC_SEASON + codes[0];
        let season_override = TC_SEASON_NAMES
            .iter()
            .find(|(sc, _, _, _, _)| *sc == codes[0]);

        let (name, groups) = if let Some(&(_, tid, target, stat, n)) = season_override {
            (
                format!("TEAMCOLOR_ATTRIBUTES_TEAM{}", tid),
                special_groups(target, stat, n, 5),
            )
        } else {
            (format!("Season {}", label), standard_groups())
        };

        colors.push(TeamColorDef {
            teamcolorid: cid,
            color_type: "special".to_string(),
            teamid: json!(cid.to_string()),
            name,
            description: format!("Pemain season {}", label),
            boostgroups: groups,
            grade: None,
        });
    }

    // 7. 78 Named Specials
    for s in TC_NAMED_SPECIALS.iter() {
        let name = if s.cid == 34024 {
            "TEAMCOLOR_ATTRIBUTES_EVOLVE34024".to_string()
        } else {
            format!("TEAMCOLOR_ATTRIBUTES_TEAM{}", s.cid)
        };

        let groups = special_groups(s.target, s.stat, s.amount, s.min_players);

        colors.push(TeamColorDef {
            teamcolorid: s.cid,
            color_type: "special".to_string(),
            teamid: json!(s.cid.to_string()),
            name,
            description: s.desc.to_string(),
            boostgroups: groups,
            grade: None,
        });
    }

    colors
}

// ---------------------------------------------------------------------------
// Active Team Color Evaluation
// ---------------------------------------------------------------------------

/// Evaluate active team colors for a squad's starting XI
pub fn active_teamcolors(
    squad_positions: &[SquadPosition],
    players: &[Player],
    fifadb: &FifaDb,
) -> Vec<ActiveTeamColor> {
    let mut out = Vec::new();
    let starters: Vec<&SquadPosition> = squad_positions
        .iter()
        .filter(|p| p.pos < POS_SUB)
        .collect();

    if starters.is_empty() {
        return out;
    }

    let spids: Vec<u32> = starters.iter().map(|p| p.spid).collect();
    let (cards, hist) = fifadb.tc_static();

    // Map distinct player id to avoid counting duplicates
    let mut club_counts: HashMap<u32, u32> = HashMap::new();
    let mut nation_counts: HashMap<u32, u32> = HashMap::new();
    let mut season_counts: HashMap<u32, u32> = HashMap::new();
    let mut seen_playerids: HashSet<u32> = HashSet::new();

    for &spid in &spids {
        if let Some(&(pid, teamid, nation)) = cards.get(&spid) {
            if seen_playerids.insert(pid) {
                // Primary club & historical clubs
                let mut clubs = HashSet::new();
                if teamid > 0 {
                    clubs.insert(teamid);
                }
                if let Some(past) = hist.get(&pid) {
                    clubs.extend(past.iter().copied());
                }

                for c in clubs {
                    *club_counts.entry(c).or_default() += 1;
                }

                if nation > 0 {
                    *nation_counts.entry(nation).or_default() += 1;
                }

                let season = spid / 1_000_000;
                *season_counts.entry(season).or_default() += 1;
            }
        }
    }

    let all_colors = get_all_team_colors();

    // 1. Evaluate Generic Club
    for (&club_id, &count) in &club_counts {
        if let Some(tier) = tc_tier(&club_groups(), count) {
            out.push(ActiveTeamColor {
                teamcolorid: TC_CLUB,
                boostindex: tier,
                teamid: json!(club_id),
            });
        }
    }

    // 2. Evaluate Generic Nation
    for (&nation_id, &count) in &nation_counts {
        if let Some(tier) = tc_tier(&standard_groups(), count) {
            out.push(ActiveTeamColor {
                teamcolorid: TC_NATION,
                boostindex: tier,
                teamid: json!(nation_id),
            });
        }
    }

    // 3. Evaluate Named Clubs & Nations
    for c in &all_colors {
        if c.teamcolorid == TC_CLUB || c.teamcolorid == TC_NATION || c.color_type == "grade" {
            continue;
        }

        let count = match c.color_type.as_str() {
            "club" => {
                let tid = c.teamid.as_u64().unwrap_or(0) as u32;
                club_counts.get(&tid).copied().unwrap_or(0)
            }
            "nation" => {
                let nid = c.teamid.as_u64().unwrap_or(0) as u32;
                nation_counts.get(&nid).copied().unwrap_or(0)
            }
            _ => 0,
        };

        if count > 0 {
            if let Some(tier) = tc_tier(&c.boostgroups, count) {
                out.push(ActiveTeamColor {
                    teamcolorid: c.teamcolorid,
                    boostindex: tier,
                    teamid: c.teamid.clone(),
                });
            }
        }
    }

    // 4. Evaluate Grade Enhancement Colors
    let player_grades: HashMap<String, u32> = players
        .iter()
        .map(|p| (p.iid(), p.grade))
        .collect();

    let starter_grades: Vec<u32> = starters
        .iter()
        .map(|s| {
            s.id.as_ref()
                .and_then(|id| player_grades.get(id))
                .or_else(|| player_grades.get(&s.spid.to_string()))
                .copied()
                .unwrap_or(0)
        })
        .collect();

    for &(cid, g, _, _) in TC_GRADE.iter() {
        let qualified = starter_grades.iter().filter(|&&grade| grade >= g).count() as u32;
        if let Some(color_def) = all_colors.iter().find(|c| c.teamcolorid == cid) {
            if let Some(tier) = tc_tier(&color_def.boostgroups, qualified) {
                out.push(ActiveTeamColor {
                    teamcolorid: cid,
                    boostindex: tier,
                    teamid: json!(-1),
                });
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_total_188_rules() {
        let colors = get_all_team_colors();
        assert_eq!(colors.len(), 188);

        let mut types = HashMap::new();
        for c in &colors {
            *types.entry(c.color_type.clone()).or_insert(0) += 1;
        }

        assert_eq!(types.get("club"), Some(&(1 + 27)));
        assert_eq!(types.get("nation"), Some(&(1 + 30)));
        assert_eq!(types.get("grade"), Some(&11));
        assert_eq!(types.get("special"), Some(&(40 + 78)));
    }

    #[test]
    fn test_tc_tier_matching() {
        let groups = club_groups();
        assert_eq!(tc_tier(&groups, 3), None);
        assert_eq!(tc_tier(&groups, 4), Some(0));
        assert_eq!(tc_tier(&groups, 5), Some(0));
        assert_eq!(tc_tier(&groups, 6), Some(1));
        assert_eq!(tc_tier(&groups, 9), Some(2));
        assert_eq!(tc_tier(&groups, 11), Some(3));
    }
}
