use fo3_server::blaze::server::run_test_client;
use fo3_server::blaze::DEFAULT_REDIR_PORT;
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    let port: u16 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_REDIR_PORT);

    println!("Connecting to Blaze server at 127.0.0.1:{}...", port);
    if let Err(e) = run_test_client("127.0.0.1", port) {
        eprintln!("test_client failed: {}", e);
        process::exit(1);
    }
}
