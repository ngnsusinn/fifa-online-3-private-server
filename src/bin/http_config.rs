use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let args: Vec<String> = env::args().collect();
    let tls = args.iter().any(|a| a == "--tls");
    let mut port = if tls { 443 } else { 8080 };
    for arg in args.iter().skip(1) {
        if !arg.starts_with("--") {
            if let Ok(p) = arg.parse::<u16>() {
                port = p;
                break;
            }
        }
    }

    let admin_port_str = env::var("FO3_ADMIN_PORT").unwrap_or_else(|_| "8088".to_string());
    let admin_port: u16 = admin_port_str.parse().unwrap_or(8088);

    if admin_port != 0 {
        tokio::spawn(async move {
            if let Err(e) = fo3_server::admin::run_admin_server(admin_port).await {
                eprintln!("[admin] error: {}", e);
            }
        });
    }

    if tls && port != 8080 {
        tokio::spawn(async move {
            if let Err(e) = fo3_server::http::server::run_http_server(8080, false).await {
                eprintln!("[http_config] http (port 8080) error: {}", e);
            }
        });
    }

    tokio::select! {
        res = fo3_server::http::server::run_http_server(port, tls) => {
            if let Err(e) = res {
                eprintln!("[http_config] server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n[http_config] shutting down...");
        }
    }

    Ok(())
}
