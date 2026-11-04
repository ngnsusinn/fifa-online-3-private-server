use byteorder::{ByteOrder, LittleEndian};
use cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub const DEFAULT_PORT: u16 = 28002;
pub const KEY: &[u8] = b"kueec";

type BlowfishCbcEnc = cbc::Encryptor<blowfish::Blowfish>;
type BlowfishCbcDec = cbc::Decryptor<blowfish::Blowfish>;

pub fn encode(obj: &Value) -> Result<Vec<u8>, String> {
    let mut pt = serde_json::to_vec(obj).map_err(|e| e.to_string())?;
    pt.push(b'\n');

    // PKCS#5/7 padding to 8-byte block size
    let pad = 8 - (pt.len() % 8);
    pt.resize(pt.len() + pad, pad as u8);

    let iv = [0u8; 8];
    let mut cipher = BlowfishCbcEnc::new_from_slices(KEY, &iv).map_err(|e| e.to_string())?;
    let mut ct = pt.clone();
    for chunk in ct.chunks_mut(8) {
        cipher.encrypt_block_mut(chunk.into());
    }

    let mut out = Vec::with_capacity(4 + ct.len());
    let mut hdr = [0u8; 4];
    LittleEndian::write_u16(&mut hdr[0..2], ct.len() as u16);
    LittleEndian::write_u16(&mut hdr[2..4], 2); // type = 2
    out.extend_from_slice(&hdr);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn decode(body: &[u8]) -> Result<Value, String> {
    if body.is_empty() || body.len() % 8 != 0 {
        return Err("invalid body length for Blowfish-CBC (must be multiple of 8)".into());
    }
    let iv = [0u8; 8];
    let mut cipher = BlowfishCbcDec::new_from_slices(KEY, &iv).map_err(|e| e.to_string())?;
    let mut pt = body.to_vec();
    for chunk in pt.chunks_mut(8) {
        cipher.decrypt_block_mut(chunk.into());
    }

    let pad = *pt.last().ok_or("empty decrypted buffer")? as usize;
    if pad == 0 || pad > 8 || pad > pt.len() {
        return Err("invalid PKCS padding".into());
    }
    let valid_len = pt.len() - pad;
    let s = String::from_utf8_lossy(&pt[..valid_len]);
    serde_json::from_str(s.trim()).map_err(|e| e.to_string())
}

pub fn replies(msg: &Value) -> Vec<Value> {
    if let Some(cmd) = msg.get("cmd").and_then(|c| c.as_str()) {
        match cmd {
            "auth" => vec![json!({"cmd": "auth", "result": true})],
            "ping" => vec![json!({"cmd": "ping"})],
            "signon" => vec![json!({"cmd": "sign", "result": true})],
            _ => vec![],
        }
    } else {
        vec![]
    }
}

pub fn handle_client(mut stream: TcpStream) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "unknown".into());
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let mut buf = Vec::new();
    let mut temp = [0u8; 4096];

    loop {
        match stream.read(&mut temp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&temp[..n]);
                while buf.len() >= 4 {
                    let body_len = LittleEndian::read_u16(&buf[0..2]) as usize;
                    let ty = LittleEndian::read_u16(&buf[2..4]);
                    if buf.len() < 4 + body_len {
                        break;
                    }
                    let body = &buf[4..4 + body_len];
                    if ty == 2 {
                        if let Ok(msg) = decode(body) {
                            if msg.get("cmd").and_then(|c| c.as_str()) != Some("ping") {
                                println!("[messenger] <- {:?}", msg);
                            }
                            for reply in replies(&msg) {
                                if let Ok(enc_reply) = encode(&reply) {
                                    if reply.get("cmd").and_then(|c| c.as_str()) != Some("ping") {
                                        println!("[messenger] -> {:?}", reply);
                                    }
                                    let _ = stream.write_all(&enc_reply);
                                }
                            }
                        }
                    }
                    buf.drain(0..4 + body_len);
                }
            }
            Err(_) => break,
        }
    }
    println!("[messenger] closed {}", peer);
}

pub fn run_server(port: u16, running: Arc<AtomicBool>) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[messenger] failed to bind {}: {}", addr, e);
            return;
        }
    };
    let _ = listener.set_nonblocking(true);
    println!("[messenger] listening on {}", addr);

    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                thread::spawn(move || handle_client(stream));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("[messenger] accept error: {}", e);
                break;
            }
        }
    }
}

