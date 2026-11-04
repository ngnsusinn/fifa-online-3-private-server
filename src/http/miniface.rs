use crate::http::routes;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Embedded 128x128 RGBA PNG fallback image for players whose face cannot be resolved.
pub const DEFAULT_PLAYER_PNG: &[u8] = include_bytes!("../../assets/default_player.png");

/// Cached set of player IDs known to fail online fetch, avoiding repeated 3s timeouts.
static FAILED_FETCH_PIDS: OnceLock<Mutex<HashSet<u64>>> = OnceLock::new();

pub fn is_failed_pid(pid: u64) -> bool {
    let mutex = FAILED_FETCH_PIDS.get_or_init(|| Mutex::new(HashSet::new()));
    mutex.lock().map(|set| set.contains(&pid)).unwrap_or(false)
}

pub fn mark_failed_pid(pid: u64) {
    let mutex = FAILED_FETCH_PIDS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut set) = mutex.lock() {
        set.insert(pid);
    }
}

pub fn clear_failed_pids() {
    let mutex = FAILED_FETCH_PIDS.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut set) = mutex.lock() {
        set.clear();
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PlayerFaceRequest<'a> {
    pub dir: &'a str,
    pub spid: u64,
    pub base_pid: u64,
}

/// Parses a CDN relative path if it matches a player face request.
/// Formats: `externalAssets/players/p<digits>.png` or `players/p<digits>.png`.
/// Extracts `spid` and computes `base_pid = spid % 1_000_000`.
pub fn parse_player_face_request(rel: &str) -> Option<PlayerFaceRequest<'_>> {
    if rel.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
        return None;
    }

    let (dir, filename) = match rel.rfind('/') {
        Some(idx) => (&rel[..idx], &rel[idx + 1..]),
        None => ("", rel),
    };

    if !dir.ends_with("players") && !dir.contains("players/") {
        return None;
    }

    if !filename.starts_with('p') || !filename.ends_with(".png") {
        return None;
    }

    let digits_str = &filename[1..filename.len() - 4];
    if digits_str.is_empty() {
        return None;
    }

    let spid = digits_str.parse::<u64>().ok()?;
    let base_pid = spid % 1_000_000;

    Some(PlayerFaceRequest {
        dir,
        spid,
        base_pid,
    })
}

/// Resolves disk candidates for a player face by PID.
pub fn find_player_face_on_disk(dir: &str, pid: u64) -> Option<Vec<u8>> {
    let filename = format!("p{}.png", pid);
    let rel_with_dir = format!("{}/{}", dir, filename);

    let candidates = [
        routes::get_assets_dir().join(&rel_with_dir),
        routes::get_assets_dir().join("externalAssets").join("players").join(&filename),
        routes::get_assets_dir().join("players").join(&filename),
        Path::new("FO3VN").join("_cache").join("cdn").join("externalAssets").join("players").join(&filename),
        Path::new("FO3VN").join("_cache").join("games").join("fo3sg").join("externalAssets").join(&filename),
        Path::new("FO3VN").join("_cache").join("games").join("fo3sg").join("externalAssets").join("players").join(&filename),
        Path::new("FO3VN").join("data").join("ui").join("externalAssets").join("players").join(&filename),
        Path::new("FO3VN").join("data").join("ui").join("players").join(&filename),
        Path::new("cdn").join("externalAssets").join("players").join(&filename),
        Path::new("cdn").join("players").join(&filename),
    ];

    for path in &candidates {
        if path.is_file() {
            if let Ok(data) = fs::read(path) {
                if !data.is_empty() {
                    return Some(data);
                }
            }
        }
    }

    None
}

/// Gets the directory where newly fetched faces should be cached.
pub fn get_players_cache_dir() -> PathBuf {
    let assets = routes::get_assets_dir();
    let ext_players = assets.join("externalAssets").join("players");
    if ext_players.exists() {
        return ext_players;
    }
    let direct_players = assets.join("players");
    if direct_players.exists() {
        return direct_players;
    }
    ext_players
}

/// Saves downloaded face bytes to the cache directory.
pub fn save_to_cache(pid: u64, data: &[u8]) {
    let cache_dir = get_players_cache_dir();
    let _ = fs::create_dir_all(&cache_dir);
    let file_path = cache_dir.join(format!("p{}.png", pid));
    let _ = fs::write(&file_path, data);
}

/// Base URL for fifaaddict CDN. Can be overridden via `FO3_FIFAADDICT_URL` for testing/mirroring.
pub fn get_fifaaddict_base_url() -> String {
    std::env::var("FO3_FIFAADDICT_URL")
        .unwrap_or_else(|_| "https://fifaaddict.com/fo3img/players".to_string())
}

/// Fetches player face from fifaaddict with a short ~3s timeout.
pub fn fetch_from_fifaaddict(pid: u64) -> Option<Vec<u8>> {
    let base_url = get_fifaaddict_base_url();
    let url = format!("{}/p{}.png", base_url.trim_end_matches('/'), pid);

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(3)))
            .build(),
    );

    let mut resp = agent
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) FO3Server/1.0")
        .call()
        .ok()?;

    if resp.status().as_u16() != 200 {
        return None;
    }

    let mut bytes = Vec::new();
    use std::io::Read;
    resp.body_mut().as_reader().read_to_end(&mut bytes).ok()?;

    if bytes.len() < 8 || !bytes.starts_with(b"\x89PNG") {
        return None;
    }

    Some(bytes)
}

/// Main entry point for player miniface requests:
/// 1. If file exists on disk, return it.
/// 2. If not on disk, try base player ID (spid % 1_000_000) on disk.
/// 3. If still not found, try fetching from fifaaddict online (first spid, then base_pid).
///    If download succeeds, save to cache directory for future reuse.
/// 4. If failed or offline, return default fallback image (128x128 PNG).
pub fn handle_miniface_request(rel: &str) -> Option<(&'static str, Vec<u8>)> {
    let req = parse_player_face_request(rel)?;

    // 1. Check if the exact requested file exists on disk
    if let Some(data) = find_player_face_on_disk(req.dir, req.spid) {
        return Some(("image/png", data));
    }

    // 2. If not on disk, try base player ID (base_pid = spid % 1_000_000) on disk
    if req.base_pid != req.spid {
        if let Some(data) = find_player_face_on_disk(req.dir, req.base_pid) {
            return Some(("image/png", data));
        }
    }

    // 3. If still not on disk, try online fetch from fifaaddict
    if req.spid != req.base_pid && !is_failed_pid(req.spid) {
        if let Some(data) = fetch_from_fifaaddict(req.spid) {
            save_to_cache(req.spid, &data);
            return Some(("image/png", data));
        } else {
            mark_failed_pid(req.spid);
        }
    }

    if !is_failed_pid(req.base_pid) {
        if let Some(data) = fetch_from_fifaaddict(req.base_pid) {
            save_to_cache(req.base_pid, &data);
            return Some(("image/png", data));
        } else {
            mark_failed_pid(req.base_pid);
        }
    }

    // 4. Fallback to default 128x128 PNG
    Some(("image/png", DEFAULT_PLAYER_PNG.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_player_face_request() {
        // Valid player face requests
        let r1 = parse_player_face_request("externalAssets/players/p67179846.png").unwrap();
        assert_eq!(r1.dir, "externalAssets/players");
        assert_eq!(r1.spid, 67179846);
        assert_eq!(r1.base_pid, 179846);

        let r2 = parse_player_face_request("players/p179846.png").unwrap();
        assert_eq!(r2.dir, "players");
        assert_eq!(r2.spid, 179846);
        assert_eq!(r2.base_pid, 179846);

        let r3 = parse_player_face_request("externalAssets/players/p1.png").unwrap();
        assert_eq!(r3.dir, "externalAssets/players");
        assert_eq!(r3.spid, 1);
        assert_eq!(r3.base_pid, 1);

        // Non-player face requests must return None
        assert!(parse_player_face_request("externalAssets/players/_timestamp").is_none());
        assert!(parse_player_face_request("externalAssets/players/_timestamp.lst").is_none());
        assert!(parse_player_face_request("externalAssets/players/p.png").is_none());
        assert!(parse_player_face_request("externalAssets/players/p_abc.png").is_none());
        assert!(parse_player_face_request("externalAssets/crests/large/s36.png").is_none());
        assert!(parse_player_face_request("externalAssets/language_eng.big").is_none());
        assert!(parse_player_face_request("externalAssets/movie/EA.usm").is_none());
        assert!(parse_player_face_request("../secret.txt").is_none());
    }

    #[test]
    fn test_base_pid_calculation() {
        assert_eq!(67179846 % 1_000_000, 179846);
        assert_eq!(26007518 % 1_000_000, 7518);
        assert_eq!(24199108 % 1_000_000, 199108);
        assert_eq!(53152729 % 1_000_000, 152729);
        assert_eq!(179846 % 1_000_000, 179846);
        assert_eq!(1 % 1_000_000, 1);
    }

    #[test]
    fn test_default_player_png_validity() {
        assert!(DEFAULT_PLAYER_PNG.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(DEFAULT_PLAYER_PNG.len() >= 24);
        let width = u32::from_be_bytes(DEFAULT_PLAYER_PNG[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(DEFAULT_PLAYER_PNG[20..24].try_into().unwrap());
        assert_eq!(width, 128);
        assert_eq!(height, 128);
    }

    #[test]
    fn test_handle_miniface_existing_face() {
        // p1.png exists on disk in FO3VN cache
        let res = handle_miniface_request("externalAssets/players/p1.png");
        assert!(res.is_some());
        let (ctype, data) = res.unwrap();
        assert_eq!(ctype, "image/png");
        assert!(data.starts_with(b"\x89PNG"));
    }

    #[test]
    fn test_handle_miniface_fallback_to_base_face_on_disk() {
        // p67179846.png is not on disk, but p179846.png is on disk!
        let base_data = find_player_face_on_disk("externalAssets/players", 179846).unwrap();
        let res = handle_miniface_request("externalAssets/players/p67179846.png");
        assert!(res.is_some());
        let (ctype, data) = res.unwrap();
        assert_eq!(ctype, "image/png");
        // Should match the base player image
        assert_eq!(data, base_data);
    }

    #[test]
    fn test_handle_miniface_fallback_to_default_png() {
        // Unknown PID with failed marker set to bypass network delay
        mark_failed_pid(999998);
        mark_failed_pid(888999998);

        let res = handle_miniface_request("externalAssets/players/p888999998.png");
        assert!(res.is_some());
        let (ctype, data) = res.unwrap();
        assert_eq!(ctype, "image/png");
        assert_eq!(data, DEFAULT_PLAYER_PNG);
    }
}
