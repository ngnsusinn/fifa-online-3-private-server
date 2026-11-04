use fo3_server::blaze::server::BlazeServer;
use fo3_server::blaze::{DEFAULT_NOTI_PORT, DEFAULT_REDIR_PORT};
use std::env;
use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();
    let redir_port: u16 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_REDIR_PORT);
    let noti_port: u16 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_NOTI_PORT);

    let capture_dir = PathBuf::from("captures");
    println!(
        "[server] Starting Blaze server (redir_port={}, noti_port={}, captures={:?})",
        redir_port, noti_port, capture_dir
    );

    let server = match BlazeServer::bind(redir_port, noti_port, capture_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[server] Failed to bind Blaze server: {}", e);
            process::exit(1);
        }
    };

    let running = Arc::new(AtomicBool::new(true));
    let running_server = match server.start(running.clone()) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("[server] Failed to start Blaze server threads: {}", e);
            process::exit(1);
        }
    };

    println!(
        "[server] Blaze server running. Redirector on 0.0.0.0:{}, Notification on 0.0.0.0:{}",
        running_server.redir_port, running_server.noti_port
    );
    println!("[server] Waiting for client connections (press Ctrl+C to stop)...");

    while running.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
    }
}
