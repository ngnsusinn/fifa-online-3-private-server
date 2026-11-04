use fo3_server::messenger::{decode, encode, run_server, DEFAULT_PORT};
use serde_json::json;
use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn selftest() {
    let m = json!({"cmd": "auth", "sessionid": ""});
    let f = encode(&m).expect("encode failed");
    assert!(f.len() >= 4);
    let ln = u16::from_le_bytes([f[0], f[1]]) as usize;
    let ty = u16::from_le_bytes([f[2], f[3]]);
    assert_eq!(ty, 2);
    assert_eq!(ln, f.len() - 4);
    assert_eq!(ln % 8, 0);

    let dec = decode(&f[4..]).expect("decode failed");
    assert_eq!(dec, m);

    let real_hex = "8b412a58950cd8431cb7b2e7473b61a8534a576ac50317310d047b5400622951";
    let mut real_bytes = Vec::new();
    for i in (0..real_hex.len()).step_by(2) {
        real_bytes.push(u8::from_str_radix(&real_hex[i..i + 2], 16).unwrap());
    }
    let dec_real = decode(&real_bytes).expect("decode real failed");
    assert_eq!(dec_real, m);
    println!("messenger selftest OK");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        selftest();
        return;
    }

    let mut port = DEFAULT_PORT;
    for arg in args.iter().skip(1) {
        if !arg.starts_with("--") {
            if let Ok(p) = arg.parse::<u16>() {
                port = p;
                break;
            }
        }
    }

    let running = Arc::new(AtomicBool::new(true));
    let r_clone = running.clone();

    let server_thread = std::thread::spawn(move || {
        run_server(port, r_clone);
    });

    let _ = tokio::signal::ctrl_c().await;
    println!("\n[messenger] shutting down...");
    running.store(false, Ordering::Relaxed);
    let _ = server_thread.join();
}
