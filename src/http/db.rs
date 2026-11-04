use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

use crate::http::constants::{POS_ROLE, ROLE_WEIGHTS};

/// Generates the SQL CASE expression for POSOVR (client rating based on preferredposition1)
pub fn sql_posovr_expr() -> String {
    let mut parts = Vec::new();
    for (i, &role) in POS_ROLE.iter().enumerate() {
        let weights = ROLE_WEIGHTS
            .iter()
            .find(|(r, _)| *r == role)
            .map(|(_, w)| *w)
            .unwrap_or(&[]);

        let sum_terms: Vec<String> = weights
            .iter()
            .map(|(k, w)| format!("{} * p.{}", w, k))
            .collect();

        if !sum_terms.is_empty() {
            parts.push(format!("WHEN {} THEN ({}) / 100", i, sum_terms.join(" + ")));
        }
    }
    format!("(CASE p.preferredposition1 {} END)", parts.join(" "))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardSearchResult {
    pub spid: u32,
    pub playerid: u32,
    pub name: String,
    pub overallrating: u32,
    pub client_ovr: u32,
    pub pos: u32,
    pub leagueid: u32,
    pub teamid: u32,
    pub nationality: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CardSearchFilter {
    pub query: Option<String>,
    pub min_ovr: Option<u32>,
    pub max_ovr: Option<u32>,
    pub leagueid: Option<u32>,
    pub teamid: Option<u32>,
    pub nationality: Option<u32>,
    pub season: Option<u32>,
    pub pos: Option<u32>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Clone)]
pub struct FifaDb {
    path: PathBuf,
    conn: Arc<Mutex<Option<Connection>>>,
}

impl FifaDb {
    /// Create a new FifaDb pointing to `path`. If file does not exist, connection remains None.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let conn = if path.exists() {
            match Connection::open(&path) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("Failed to open fifadb at {:?}: {}", path, e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    /// Open default location: `gamedata/fifadb.sqlite`
    pub fn open_default() -> Self {
        let path = std::env::var("FO3_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("gamedata/fifadb.sqlite"));
        Self::new(path)
    }

    /// For tests or custom initialization
    pub fn from_connection(conn: Connection) -> Self {
        Self {
            path: PathBuf::new(),
            conn: Arc::new(Mutex::new(Some(conn))),
        }
    }

    pub fn is_available(&self) -> bool {
        self.conn.lock().unwrap().is_some()
    }

    /// Re-check or reload DB connection
    pub fn reload(&self) -> bool {
        let mut guard = self.conn.lock().unwrap();
        if self.path.exists() {
            match Connection::open(&self.path) {
                Ok(c) => {
                    *guard = Some(c);
                    true
                }
                Err(_) => {
                    *guard = None;
                    false
                }
            }
        } else {
            *guard = None;
            false
        }
    }

    /// Get client OVR and DB overallrating for multiple SPIDs: {spid: (client_ovr, overallrating)}
    pub fn card_ratings(&self, spids: &[u32]) -> HashMap<u32, (u32, u32)> {
        let mut result = HashMap::new();
        if spids.is_empty() {
            return result;
        }

        let guard = self.conn.lock().unwrap();
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => return result,
        };

        let posovr = sql_posovr_expr();

        for chunk in spids.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT p.spid, {}, p.overallrating FROM players p WHERE p.spid IN ({})",
                posovr, placeholders
            );

            let params: Vec<rusqlite::types::Value> =
                chunk.iter().map(|&s| (s as i64).into()).collect();

            if let Ok(mut stmt) = conn.prepare(&sql) {
                let query_res = stmt.query_map(params_from_iter(params.clone()), |row| {
                    let spid: u32 = row.get(0)?;
                    let client_ovr: Option<u32> = row.get(1)?;
                    let ovr: u32 = row.get(2)?;
                    Ok((spid, (client_ovr.unwrap_or(ovr), ovr)))
                });
                if let Ok(rows) = query_res {
                    for r in rows.flatten() {
                        result.insert(r.0, r.1);
                    }
                    continue;
                }
            }

            // Fallback if full stat query failed
            let fallback_sql = format!(
                "SELECT p.spid, p.overallrating, p.overallrating FROM players p WHERE p.spid IN ({})",
                placeholders
            );
            if let Ok(mut stmt) = conn.prepare(&fallback_sql) {
                let query_res = stmt.query_map(params_from_iter(params), |row| {
                    let spid: u32 = row.get(0)?;
                    let client_ovr: Option<u32> = row.get(1)?;
                    let ovr: u32 = row.get(2)?;
                    Ok((spid, (client_ovr.unwrap_or(ovr), ovr)))
                });
                if let Ok(rows) = query_res {
                    for r in rows.flatten() {
                        result.insert(r.0, r.1);
                    }
                }
            }
        }

        result
    }

    /// Get client OVR for multiple SPIDs
    pub fn card_ovrs(&self, spids: &[u32]) -> HashMap<u32, u32> {
        self.card_ratings(spids)
            .into_iter()
            .map(|(s, (client_ovr, _))| (s, client_ovr))
            .collect()
    }

    /// Get client OVR for single SPID, or 0 if missing/not found
    pub fn card_ovr(&self, spid: u32) -> u32 {
        self.card_ovrs(&[spid]).get(&spid).copied().unwrap_or(0)
    }

    /// Get player names from nameseng (preferred) or namesind
    pub fn get_player_names(&self, spids: &[u32]) -> HashMap<u32, String> {
        let mut result = HashMap::new();
        if spids.is_empty() {
            return result;
        }

        let guard = self.conn.lock().unwrap();
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => return result,
        };

        for chunk in spids.chunks(500) {
            let placeholders = vec!["?"; chunk.len()].join(",");
            let sql = format!(
                "SELECT p.spid, COALESCE(n.full_eng, n.full, n.short)
                 FROM players p
                 JOIN nameseng n ON n.playerid = p.playerid
                 WHERE p.spid IN ({})",
                placeholders
            );

            let params: Vec<rusqlite::types::Value> =
                chunk.iter().map(|&s| (s as i64).into()).collect();

            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(rows) = stmt.query_map(params_from_iter(params), |row| {
                    let spid: u32 = row.get(0)?;
                    let name: Option<String> = row.get(1)?;
                    Ok((spid, name.unwrap_or_default()))
                }) {
                    for r in rows.flatten() {
                        result.insert(r.0, r.1);
                    }
                }
            }
        }

        result
    }

    /// Get static info for team colors:
    /// (cards: {spid: (playerid, teamid, nationality)}, history: {playerid: {teamid}})
    pub fn tc_static(&self) -> (HashMap<u32, (u32, u32, u32)>, HashMap<u32, HashSet<u32>>) {
        let mut cards = HashMap::new();
        let mut hist: HashMap<u32, HashSet<u32>> = HashMap::new();

        let guard = self.conn.lock().unwrap();
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => return (cards, hist),
        };

        if let Ok(mut stmt) =
            conn.prepare("SELECT spid, playerid, teamid, nationality FROM players")
        {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, u32>(3)?,
                ))
            }) {
                for r in rows.flatten() {
                    cards.insert(r.0, (r.1, r.2, r.3));
                }
            }
        }

        if let Ok(mut stmt) = conn.prepare("SELECT playerid, teamid FROM playerhistory") {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, u32>(1)?))
            }) {
                for r in rows.flatten() {
                    hist.entry(r.0).or_default().insert(r.1);
                }
            }
        }

        (cards, hist)
    }

    /// Every transferable card that appears on Market
    pub fn get_market_spids(&self) -> HashSet<u32> {
        let mut result = HashSet::new();
        let guard = self.conn.lock().unwrap();
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => return result,
        };

        let sql = "SELECT p.spid FROM players p
                   JOIN permid m ON m.spid = p.spid AND m.transferable = 1
                   WHERE p.spid >= 1000000
                   AND p.playerid IN (SELECT playerid FROM namesind)";

        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map([], |row| row.get::<_, u32>(0)) {
                for r in rows.flatten() {
                    result.insert(r);
                }
            }
        }

        result
    }

    /// Search cards with optional filtering
    pub fn search_cards(&self, filter: &CardSearchFilter) -> Vec<CardSearchResult> {
        let mut results = Vec::new();
        let guard = self.conn.lock().unwrap();
        let conn = match guard.as_ref() {
            Some(c) => c,
            None => return results,
        };

        let posovr = sql_posovr_expr();
        let mut where_clauses = Vec::new();
        let mut params: Vec<rusqlite::types::Value> = Vec::new();

        if let Some(ref q) = filter.query {
            where_clauses.push("(n.full_eng LIKE ? OR n.full LIKE ? OR n.short LIKE ?)".to_string());
            let pattern = format!("%{}%", q);
            params.push(pattern.clone().into());
            params.push(pattern.clone().into());
            params.push(pattern.into());
        }

        if let Some(min_ovr) = filter.min_ovr {
            where_clauses.push(format!("{} >= ?", posovr));
            params.push((min_ovr as i64).into());
        }

        if let Some(max_ovr) = filter.max_ovr {
            where_clauses.push(format!("{} <= ?", posovr));
            params.push((max_ovr as i64).into());
        }

        if let Some(lid) = filter.leagueid {
            where_clauses.push("p.leagueid = ?".to_string());
            params.push((lid as i64).into());
        }

        if let Some(tid) = filter.teamid {
            where_clauses.push("p.teamid = ?".to_string());
            params.push((tid as i64).into());
        }

        if let Some(nat) = filter.nationality {
            where_clauses.push("p.nationality = ?".to_string());
            params.push((nat as i64).into());
        }

        if let Some(season) = filter.season {
            where_clauses.push("(p.spid / 1000000) = ?".to_string());
            params.push((season as i64).into());
        }

        if let Some(pos) = filter.pos {
            where_clauses.push("p.preferredposition1 = ?".to_string());
            params.push((pos as i64).into());
        }

        let where_sql = if where_clauses.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let limit_sql = match (filter.limit, filter.offset) {
            (Some(lim), Some(off)) => format!("LIMIT {} OFFSET {}", lim, off),
            (Some(lim), None) => format!("LIMIT {}", lim),
            _ => "LIMIT 100".to_string(),
        };

        let sql = format!(
            "SELECT p.spid, p.playerid, COALESCE(n.full_eng, n.full, n.short, ''),
                    p.overallrating, {}, p.preferredposition1, p.leagueid, p.teamid, p.nationality
             FROM players p
             LEFT JOIN nameseng n ON n.playerid = p.playerid
             {}
             ORDER BY {} DESC
             {}",
            posovr, where_sql, posovr, limit_sql
        );

        let fallback_sql = format!(
            "SELECT p.spid, p.playerid, COALESCE(n.full_eng, n.full, n.short, ''),
                    p.overallrating, p.overallrating, p.preferredposition1, p.leagueid, p.teamid, p.nationality
             FROM players p
             LEFT JOIN nameseng n ON n.playerid = p.playerid
             {}
             ORDER BY p.overallrating DESC
             {}",
            where_sql, limit_sql
        );

        let query_sql = if conn.prepare(&sql).is_ok() { sql } else { fallback_sql };
        if let Ok(mut stmt) = conn.prepare(&query_sql) {
            let query_res = stmt.query_map(params_from_iter(params), |row| {
                Ok(CardSearchResult {
                    spid: row.get(0)?,
                    playerid: row.get(1)?,
                    name: row.get(2)?,
                    overallrating: row.get(3)?,
                    client_ovr: row.get::<_, Option<u32>>(4)?.unwrap_or_else(|| row.get(3).unwrap_or(0)),
                    pos: row.get(5)?,
                    leagueid: row.get(6)?,
                    teamid: row.get(7)?,
                    nationality: row.get(8)?,
                })
            });
            if let Ok(rows) = query_res {
                for r in rows.flatten() {
                    results.push(r);
                }
            }
        }

        results
    }

    /// Roster players for a given club
    pub fn get_team_roster(&self, teamid: u32) -> Vec<CardSearchResult> {
        self.search_cards(&CardSearchFilter {
            teamid: Some(teamid),
            limit: Some(50),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_fallback() {
        let db = FifaDb::new("non_existent_fifadb.sqlite");
        assert!(!db.is_available());
        assert_eq!(db.card_ovr(100), 0);
        assert!(db.card_ratings(&[100, 200]).is_empty());
        assert!(db.get_player_names(&[100]).is_empty());
        assert!(db.get_market_spids().is_empty());
        assert!(db.search_cards(&CardSearchFilter::default()).is_empty());
    }

    #[test]
    fn test_in_memory_db() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE players (
                spid INTEGER PRIMARY KEY,
                playerid INTEGER,
                teamid INTEGER,
                nationality INTEGER,
                preferredposition1 INTEGER,
                overallrating INTEGER,
                leagueid INTEGER,
                gkdiving INTEGER DEFAULT 80,
                gkhandling INTEGER DEFAULT 80,
                gkkicking INTEGER DEFAULT 80,
                gkpositioning INTEGER DEFAULT 80,
                gkreflexes INTEGER DEFAULT 80,
                reactions INTEGER DEFAULT 80
            );
            CREATE TABLE nameseng (
                playerid INTEGER PRIMARY KEY,
                full_eng TEXT,
                full TEXT,
                short TEXT
            );
            INSERT INTO players (spid, playerid, teamid, nationality, preferredposition1, overallrating, leagueid)
            VALUES (1001, 501, 241, 45, 0, 80, 53);
            INSERT INTO nameseng (playerid, full_eng) VALUES (501, 'Test Goalkeeper');"
        ).unwrap();

        let db = FifaDb::from_connection(conn);
        assert!(db.is_available());

        let ovr = db.card_ovr(1001);
        assert_eq!(ovr, 80);

        let names = db.get_player_names(&[1001]);
        assert_eq!(names.get(&1001).unwrap(), "Test Goalkeeper");

        let results = db.search_cards(&CardSearchFilter {
            teamid: Some(241),
            ..Default::default()
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Test Goalkeeper");
    }
}
