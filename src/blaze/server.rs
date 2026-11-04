use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::blaze::tdf::{
    decode_tdf, enc_int, enc_string, enc_struct, TdfValue,
};
use crate::blaze::{
    build_frame, describe, frames_and_rest, Header, CMD_CREATE_GAME, CMD_GET_SERVER_INSTANCE,
    CMD_JOIN_GAME, CMD_LOGIN, CMD_LOGIN_PERSONA, CMD_PING, CMD_PRE_AUTH, CMD_START_MATCHMAKING,
    COMP_AUTHENTICATION, COMP_GAME_MANAGER, COMP_REDIRECTOR, COMP_UTIL, MSG_MESSAGE, MSG_REPLY,
};

static CONN_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Represents an authenticated Blaze session.
#[derive(Debug, Clone)]
pub struct Session {
    pub session_key: String,
    pub user_id: u64,
    pub persona_id: u64,
    pub display_name: String,
    pub email: String,
    pub client_version: String,
    pub first_login: u64,
    pub region: String,
    pub server_version: u64,
    pub maintenance: u64,
    pub created_at: u64,
}

fn default_region() -> String {
    std::env::var("FO3_REGION").unwrap_or_else(|_| {
        let lang = std::env::var("FO3_LANGUAGE").unwrap_or_default();
        if lang.eq_ignore_ascii_case("eng") {
            return "SG".to_string();
        }
        if std::path::Path::new("FO3VN").exists()
            || std::path::Path::new(r"C:\Users\ngnsusinn\Documents\fifa-online-3-server\FO3VN").exists()
        {
            "VN".to_string()
        } else {
            "ID".to_string()
        }
    })
}

impl Default for Session {
    fn default() -> Self {
        Self {
            session_key: "LOCAL-SESSION-0001".to_string(),
            user_id: 1,
            persona_id: 1,
            display_name: "player".to_string(),
            email: "player@local".to_string(),
            client_version: "20200217".to_string(),
            first_login: 0,
            region: default_region(),
            server_version: 1,
            maintenance: 0,
            created_at: 0,
        }
    }
}

/// Thread-safe session management.
#[derive(Debug)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, Session>>,
    next_session_id: AtomicU64,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1),
        }
    }

    pub fn get_or_create_session(
        &self,
        email: Option<String>,
        display_name: Option<String>,
    ) -> Session {
        let email_str = email.unwrap_or_else(|| "player@local".to_string());
        let mut lock = self.sessions.lock().unwrap();

        // If a session with this email exists, reuse it
        for sess in lock.values() {
            if sess.email == email_str {
                return sess.clone();
            }
        }

        let sid = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session_key = format!("LOCAL-SESSION-{:04}", sid);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let session = Session {
            session_key: session_key.clone(),
            user_id: sid,
            persona_id: sid,
            display_name: display_name.unwrap_or_else(|| "player".to_string()),
            email: email_str,
            client_version: "20200217".to_string(),
            first_login: 0,
            region: default_region(),
            server_version: 1,
            maintenance: 0,
            created_at: now,
        };

        lock.insert(session_key, session.clone());
        session
    }

    pub fn create_session(
        &self,
        email: Option<String>,
        display_name: Option<String>,
    ) -> Session {
        let sid = self.next_session_id.fetch_add(1, Ordering::SeqCst);
        let session_key = format!("LOCAL-SESSION-{:04}", sid);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let session = Session {
            session_key: session_key.clone(),
            user_id: sid,
            persona_id: sid,
            display_name: display_name.unwrap_or_else(|| "player".to_string()),
            email: email.unwrap_or_else(|| "player@local".to_string()),
            client_version: "20200217".to_string(),
            first_login: 0,
            region: default_region(),
            server_version: 1,
            maintenance: 0,
            created_at: now,
        };

        let mut lock = self.sessions.lock().unwrap();
        lock.insert(session_key, session.clone());
        session
    }

    pub fn get_session(&self, key: &str) -> Option<Session> {
        let lock = self.sessions.lock().unwrap();
        lock.get(key).cloned()
    }

    pub fn update_persona(&self, key: &str, persona_name: &str) {
        let mut lock = self.sessions.lock().unwrap();
        if let Some(sess) = lock.get_mut(key) {
            sess.display_name = persona_name.to_string();
        }
    }

    pub fn update_client_version(&self, key: &str, cver: &str) {
        let mut lock = self.sessions.lock().unwrap();
        if let Some(sess) = lock.get_mut(key) {
            sess.client_version = cver.to_string();
        }
    }

    pub fn active_count(&self) -> usize {
        let lock = self.sessions.lock().unwrap();
        lock.len()
    }
}

/// Represents game state created via GameManager.
#[derive(Debug, Clone)]
pub struct GameInfo {
    pub gid: u64,
    pub name: String,
    pub game_type: u64,
    pub state: u64,
}

/// Thread-safe game management.
#[derive(Debug)]
pub struct GameManager {
    next_gid: AtomicU64,
    games: Mutex<HashMap<u64, GameInfo>>,
}

impl Default for GameManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GameManager {
    pub fn new() -> Self {
        Self {
            next_gid: AtomicU64::new(1000),
            games: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_game(&self, name: String, game_type: u64) -> GameInfo {
        let gid = self.next_gid.fetch_add(1, Ordering::SeqCst);
        let game = GameInfo {
            gid,
            name,
            game_type,
            state: 0,
        };
        let mut lock = self.games.lock().unwrap();
        lock.insert(gid, game.clone());
        game
    }

    pub fn last_game_id(&self) -> u64 {
        let cur = self.next_gid.load(Ordering::SeqCst);
        if cur > 1000 {
            cur - 1
        } else {
            1000
        }
    }

    pub fn start_matchmaking(&self, _game_type: u64) -> u64 {
        5001
    }
}

/// Shared Blaze server state.
#[derive(Debug)]
pub struct BlazeServerState {
    pub session_manager: SessionManager,
    pub game_manager: GameManager,
    pub game_host: String,
    pub game_port: u16,
    pub capture_dir: PathBuf,
    pub start_time: Instant,
}

impl BlazeServerState {
    pub fn new(game_host: String, game_port: u16, capture_dir: PathBuf) -> Self {
        Self {
            session_manager: SessionManager::new(),
            game_manager: GameManager::new(),
            game_host,
            game_port,
            capture_dir,
            start_time: Instant::now(),
        }
    }

    pub fn handle_frame(
        &self,
        hdr: &Header,
        payload: &[u8],
        conn_session_key: &mut Option<String>,
    ) -> Option<Vec<u8>> {
        match (hdr.component, hdr.command) {
            (COMP_REDIRECTOR, CMD_GET_SERVER_INSTANCE) => {
                Some(self.handle_redirector(hdr))
            }
            (COMP_AUTHENTICATION, CMD_LOGIN) => {
                Some(self.handle_login(hdr, payload, conn_session_key))
            }
            (COMP_AUTHENTICATION, CMD_LOGIN_PERSONA) => {
                Some(self.handle_login_persona(hdr, payload, conn_session_key))
            }
            (COMP_UTIL, CMD_PRE_AUTH) => {
                Some(self.handle_pre_auth(hdr, payload, conn_session_key))
            }
            (COMP_UTIL, CMD_PING) => {
                Some(self.handle_ping(hdr))
            }
            (COMP_GAME_MANAGER, CMD_CREATE_GAME) => {
                Some(self.handle_create_game(hdr, payload))
            }
            (COMP_GAME_MANAGER, CMD_JOIN_GAME) => {
                Some(self.handle_join_game(hdr))
            }
            (COMP_GAME_MANAGER, CMD_START_MATCHMAKING) => {
                Some(self.handle_start_matchmaking(hdr))
            }
            _ => None,
        }
    }

    fn handle_redirector(&self, hdr: &Header) -> Vec<u8> {
        let mut inner = Vec::new();
        inner.extend_from_slice(&enc_string("HOST", &self.game_host));
        inner.extend_from_slice(&enc_int("IP  ", ip_to_int(&self.game_host)));
        inner.extend_from_slice(&enc_int("PORT", self.game_port as u64));

        let mut body = Vec::new();
        body.extend_from_slice(&enc_struct("ADDR", &inner));
        body.extend_from_slice(&enc_int("SVID", 1));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_login(
        &self,
        hdr: &Header,
        payload: &[u8],
        conn_session_key: &mut Option<String>,
    ) -> Vec<u8> {
        let fields = decode_tdf(payload);
        let mail = find_str(&fields, "MAIL").map(|s| s.to_string());
        let session = self.session_manager.get_or_create_session(mail, Some("player".to_string()));
        *conn_session_key = Some(session.session_key.clone());

        let mut sess = Vec::new();
        sess.extend_from_slice(&enc_int("BUID", session.user_id));
        sess.extend_from_slice(&enc_string("DSNM", &session.display_name));
        sess.extend_from_slice(&enc_int("FRST", session.first_login));

        let mut body = Vec::new();
        body.extend_from_slice(&enc_string("SKEY", &session.session_key));
        body.extend_from_slice(&enc_struct("SESS", &sess));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_login_persona(
        &self,
        hdr: &Header,
        payload: &[u8],
        conn_session_key: &mut Option<String>,
    ) -> Vec<u8> {
        let fields = decode_tdf(payload);
        if let Some(pnam) = find_str(&fields, "PNAM") {
            if let Some(ref key) = conn_session_key {
                self.session_manager.update_persona(key, pnam);
            }
        }

        let session = conn_session_key
            .as_deref()
            .and_then(|k| self.session_manager.get_session(k))
            .unwrap_or_default();

        let mut pdtl = Vec::new();
        pdtl.extend_from_slice(&enc_int("PID ", session.persona_id));
        pdtl.extend_from_slice(&enc_string("DSNM", &session.display_name));
        pdtl.extend_from_slice(&enc_int("BUID", session.user_id));

        let mut body = Vec::new();
        body.extend_from_slice(&enc_struct("PDTL", &pdtl));
        body.extend_from_slice(&enc_string("SKEY", &session.session_key));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_pre_auth(
        &self,
        hdr: &Header,
        payload: &[u8],
        conn_session_key: &mut Option<String>,
    ) -> Vec<u8> {
        let fields = decode_tdf(payload);
        if let Some(cver) = find_str(&fields, "CVER") {
            if let Some(ref key) = conn_session_key {
                self.session_manager.update_client_version(key, cver);
            }
        }

        let session = conn_session_key
            .as_deref()
            .and_then(|k| self.session_manager.get_session(k))
            .unwrap_or_default();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut conf = Vec::new();
        conf.extend_from_slice(&enc_string("REGN", &session.region));
        conf.extend_from_slice(&enc_int("SVER", session.server_version));
        conf.extend_from_slice(&enc_int("MAIT", session.maintenance));

        let mut body = Vec::new();
        body.extend_from_slice(&enc_struct("CONF", &conf));
        body.extend_from_slice(&enc_int("TIME", now));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_ping(&self, hdr: &Header) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let body = enc_int("STIM", now);
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_create_game(&self, hdr: &Header, payload: &[u8]) -> Vec<u8> {
        let fields = decode_tdf(payload);
        let gnam = find_str(&fields, "GNAM").unwrap_or("local game").to_string();
        let gtyp = find_int(&fields, "GTYP").unwrap_or(0);
        let game = self.game_manager.create_game(gnam, gtyp);

        let mut body = Vec::new();
        body.extend_from_slice(&enc_int("GID ", game.gid));
        body.extend_from_slice(&enc_string("GNAM", &game.name));
        body.extend_from_slice(&enc_int("GSTT", game.state));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_join_game(&self, hdr: &Header) -> Vec<u8> {
        let gid = self.game_manager.last_game_id();
        let mut body = Vec::new();
        body.extend_from_slice(&enc_int("GID ", gid));
        body.extend_from_slice(&enc_int("JOIN", 1));
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }

    fn handle_start_matchmaking(&self, hdr: &Header) -> Vec<u8> {
        let msid = self.game_manager.start_matchmaking(0);
        let body = enc_int("MSID", msid);
        build_frame(hdr.component, hdr.command, MSG_REPLY, hdr.msgid, &body, 0)
    }
}

pub fn ip_to_int(host: &str) -> u64 {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        u32::from(ip) as u64
    } else {
        0
    }
}

pub fn find_field<'a>(fields: &'a [(String, TdfValue)], tag: &str) -> Option<&'a TdfValue> {
    fields
        .iter()
        .find(|(k, _)| k == tag || k.trim() == tag.trim())
        .map(|(_, v)| v)
}

pub fn find_int(fields: &[(String, TdfValue)], tag: &str) -> Option<u64> {
    find_field(fields, tag).and_then(|v| v.as_u64())
}

pub fn find_str<'a>(fields: &'a [(String, TdfValue)], tag: &str) -> Option<&'a str> {
    find_field(fields, tag).and_then(|v| v.as_str())
}

pub fn find_struct<'a>(
    fields: &'a [(String, TdfValue)],
    tag: &str,
) -> Option<&'a [(String, TdfValue)]> {
    find_field(fields, tag).and_then(|v| v.as_struct())
}

fn handle_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    role: String,
    state: Arc<BlazeServerState>,
) {
    let conn_id = CONN_COUNTER.fetch_add(1, Ordering::SeqCst);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let _ = create_dir_all(&state.capture_dir);
    let mut cap_path = state.capture_dir.join(format!("{}_{}.bin", role, ts));
    if cap_path.exists() {
        cap_path = state
            .capture_dir
            .join(format!("{}_{}_{}.bin", role, ts, conn_id));
    }

    println!(
        "[{:7.2}s] {}: connect from {} -> capturing to {}",
        state.start_time.elapsed().as_secs_f64(),
        role,
        addr,
        cap_path.file_name().unwrap_or_default().to_string_lossy()
    );

    let mut cap_file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&cap_path)
    {
        Ok(f) => Some(f),
        Err(e) => {
            eprintln!("[blaze] warning: cannot open capture file {:?}: {}", cap_path, e);
            None
        }
    };

    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));

    let mut buf = Vec::new();
    let mut temp = [0u8; 8192];
    let mut conn_session_key: Option<String> = None;

    loop {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => {
                if let Some(ref mut f) = cap_file {
                    let _ = f.write_all(&temp[..n]);
                    let _ = f.flush();
                }
                buf.extend_from_slice(&temp[..n]);
                let (frames, rest) = frames_and_rest(&buf);
                for (hdr, payload) in frames {
                    println!(
                        "[{:7.2}s] {}  FRAME  {}",
                        state.start_time.elapsed().as_secs_f64(),
                        role,
                        describe(&hdr, &payload)
                    );
                    if !payload.is_empty() {
                        let fields = decode_tdf(&payload);
                        if !fields.is_empty() {
                            println!(
                                "[{:7.2}s] {}  TDF    {:?}",
                                state.start_time.elapsed().as_secs_f64(),
                                role,
                                fields
                            );
                        }
                    }
                    if let Some(reply) = state.handle_frame(&hdr, &payload, &mut conn_session_key) {
                        if let Err(e) = stream.write_all(&reply) {
                            println!(
                                "[{:7.2}s] {}: {} write error ({})",
                                state.start_time.elapsed().as_secs_f64(),
                                role,
                                addr,
                                e
                            );
                            return;
                        }
                        let _ = stream.flush();
                        println!(
                            "[{:7.2}s] {}  REPLY  sent {}B for comp=0x{:04x} cmd=0x{:04x}",
                            state.start_time.elapsed().as_secs_f64(),
                            role,
                            reply.len(),
                            hdr.component,
                            hdr.command
                        );
                    }
                }
                buf = rest;
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                break;
            }
            Err(e) => {
                println!(
                    "[{:7.2}s] {}: {} closed ({})",
                    state.start_time.elapsed().as_secs_f64(),
                    role,
                    addr,
                    e
                );
                break;
            }
        }
    }

    if !buf.is_empty() {
        let hex_slice = if buf.len() > 32 {
            format!("{} ...", crate::blaze::hex::encode(&buf[..32]))
        } else {
            crate::blaze::hex::encode(&buf)
        };
        println!(
            "[{:7.2}s] {}: {} trailing undecoded bytes: {}",
            state.start_time.elapsed().as_secs_f64(),
            role,
            buf.len(),
            hex_slice
        );
    }
}

fn run_listener(
    listener: TcpListener,
    role: &'static str,
    running: Arc<AtomicBool>,
    state: Arc<BlazeServerState>,
) {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    println!(
        "[{:7.2}s] {} listening on 0.0.0.0:{}",
        state.start_time.elapsed().as_secs_f64(),
        role,
        port
    );

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, addr)) => {
                let _ = stream.set_nonblocking(false);
                let state_clone = state.clone();
                let role_str = role.to_string();
                thread::spawn(move || {
                    handle_client(stream, addr, role_str, state_clone);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                if running.load(Ordering::Relaxed) {
                    eprintln!("[blaze] accept error on {}: {}", role, e);
                }
                break;
            }
        }
    }
}

pub struct BlazeServer {
    pub redir_listener: TcpListener,
    pub noti_listener: TcpListener,
    pub redir_port: u16,
    pub noti_port: u16,
    pub state: Arc<BlazeServerState>,
}

pub struct RunningServer {
    pub redir_port: u16,
    pub noti_port: u16,
    pub running: Arc<AtomicBool>,
    pub state: Arc<BlazeServerState>,
    handles: Vec<JoinHandle<()>>,
}

impl RunningServer {
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn join(self) {
        for handle in self.handles {
            let _ = handle.join();
        }
    }
}

impl BlazeServer {
    pub fn bind(redir_port: u16, noti_port: u16, capture_dir: PathBuf) -> std::io::Result<Self> {
        let redir_addr = format!("0.0.0.0:{}", redir_port);
        let redir_listener = TcpListener::bind(&redir_addr)?;
        let bound_redir_port = redir_listener.local_addr()?.port();
        redir_listener.set_nonblocking(true)?;

        let noti_addr = format!("0.0.0.0:{}", noti_port);
        let noti_listener = TcpListener::bind(&noti_addr)?;
        let bound_noti_port = noti_listener.local_addr()?.port();
        noti_listener.set_nonblocking(true)?;

        let game_port = crate::blaze::DEFAULT_REDIR_PORT;

        let state = Arc::new(BlazeServerState::new(
            "127.0.0.1".to_string(),
            game_port,
            capture_dir,
        ));

        Ok(Self {
            redir_listener,
            noti_listener,
            redir_port: bound_redir_port,
            noti_port: bound_noti_port,
            state,
        })
    }

    pub fn start(self, running: Arc<AtomicBool>) -> std::io::Result<RunningServer> {
        let redir_port = self.redir_port;
        let noti_port = self.noti_port;
        let state = self.state.clone();

        let redir_handle = {
            let listener = self.redir_listener;
            let running = running.clone();
            let state = state.clone();
            thread::spawn(move || {
                run_listener(listener, "redirector", running, state);
            })
        };

        let noti_handle = {
            let listener = self.noti_listener;
            let running = running.clone();
            let state = state.clone();
            thread::spawn(move || {
                run_listener(listener, "notification", running, state);
            })
        };

        Ok(RunningServer {
            redir_port,
            noti_port,
            running,
            state,
            handles: vec![redir_handle, noti_handle],
        })
    }
}

/// Executes the full 8-command Blaze loopback client exchange against host:port.
pub fn run_test_client(host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect(&addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut buf = Vec::new();
    let mut pending = Vec::new();

    let mut exchange = |component: u16,
                        command: u16,
                        msgid: u32,
                        body: &[u8]|
     -> Result<(Header, Vec<(String, TdfValue)>), Box<dyn std::error::Error>> {
        let frame = build_frame(component, command, MSG_MESSAGE, msgid, body, 0);
        stream.write_all(&frame)?;
        let mut temp = [0u8; 4096];
        while pending.is_empty() {
            let n = stream.read(&mut temp)?;
            if n == 0 {
                return Err("server closed connection without replying".into());
            }
            buf.extend_from_slice(&temp[..n]);
            let (frames, rest) = frames_and_rest(&buf);
            pending.extend(frames);
            buf = rest;
        }
        let (hdr, payload) = pending.remove(0);
        let fields = decode_tdf(&payload);
        Ok((hdr, fields))
    };

    // 1) Redirector getServerInstance
    let mut req1 = Vec::new();
    req1.extend_from_slice(&enc_string("NAME", "fo3id"));
    req1.extend_from_slice(&enc_int("PLAT", 0));
    let (hdr, f) = exchange(0x0005, 0x0001, 1, &req1)?;
    println!("redirector reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0005);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 1);
    let addr_struct = find_struct(&f, "ADDR").expect("ADDR missing");
    let host_val = find_str(addr_struct, "HOST").expect("HOST missing");
    let port_val = find_int(addr_struct, "PORT").expect("PORT missing");
    assert_eq!(host_val, "127.0.0.1");
    assert!(
        port_val == 28000 || port_val == port as u64,
        "Expected PORT 28000 or {}, got {}",
        port,
        port_val
    );

    // 2) Authentication login
    let mut req2 = Vec::new();
    req2.extend_from_slice(&enc_string("MAIL", "player@local"));
    req2.extend_from_slice(&enc_string("PASS", "x"));
    let (hdr, f) = exchange(0x0001, 0x000C, 2, &req2)?;
    println!("login reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0001);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 2);
    assert_eq!(find_str(&f, "SKEY"), Some("LOCAL-SESSION-0001"));
    let sess_struct = find_struct(&f, "SESS").expect("SESS missing");
    assert_eq!(find_int(sess_struct, "BUID"), Some(1));
    assert_eq!(find_str(sess_struct, "DSNM"), Some("player"));

    // 3) Util preAuth (lobby entry config)
    let req3 = enc_string("CVER", "20200217");
    let (hdr, f) = exchange(0x0009, 0x0007, 3, &req3)?;
    println!("preAuth reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0009);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 3);
    let conf_struct = find_struct(&f, "CONF").expect("CONF missing");
    assert_eq!(find_str(conf_struct, "REGN"), Some(default_region().as_str()));

    // 4) Authentication loginPersona
    let req4 = enc_string("PNAM", "player");
    let (hdr, f) = exchange(0x0001, 0x006E, 4, &req4)?;
    println!("loginPersona reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0001);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 4);
    let pdtl_struct = find_struct(&f, "PDTL").expect("PDTL missing");
    assert_eq!(find_str(pdtl_struct, "DSNM"), Some("player"));
    assert_eq!(find_int(pdtl_struct, "BUID"), Some(1));

    // 5) Util ping
    let (hdr, f) = exchange(0x0009, 0x0002, 5, &[])?;
    println!("ping reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0009);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 5);
    assert!(find_int(&f, "STIM").is_some(), "STIM missing in ping reply");

    // 6) GameManager createGame
    let mut req6 = Vec::new();
    req6.extend_from_slice(&enc_string("GNAM", "local game"));
    req6.extend_from_slice(&enc_int("GTYP", 0));
    let (hdr, f) = exchange(0x0004, 0x0001, 6, &req6)?;
    println!("createGame reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0004);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 6);
    let gid = find_int(&f, "GID").expect("GID missing");
    assert!(gid >= 1000);

    // 7) GameManager joinGame
    let req7 = enc_int("GID ", 1000);
    let (hdr, f) = exchange(0x0004, 0x0002, 7, &req7)?;
    println!("joinGame reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0004);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 7);
    assert_eq!(find_int(&f, "JOIN"), Some(1));

    // 8) GameManager startMatchmaking
    let req8 = enc_int("GTYP", 0);
    let (hdr, f) = exchange(0x0004, 0x000A, 8, &req8)?;
    println!("startMatchmaking reply: {} {:?}", describe(&hdr, &[]), f);
    assert_eq!(hdr.component, 0x0004);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 8);
    assert!(find_int(&f, "MSID").is_some(), "MSID missing in matchmaking reply");

    println!("test_client OK — redirector + login + preAuth + persona + ping + create/join/matchmaking framed and decoded");
    Ok(())
}
