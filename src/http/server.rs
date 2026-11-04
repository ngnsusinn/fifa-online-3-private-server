use crate::http::routes::create_http_router;
use axum_server::tls_rustls::RustlsConfig;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

const CERT_PATH: &str = "certs/server.pem";
const KEY_PATH: &str = "certs/server.key";

pub async fn run_http_server(
    port: u16,
    tls: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let app = create_http_router();

    if tls {
        let (cert_pem, key_pem) = get_or_generate_cert()?;
        let rustls_config = RustlsConfig::from_pem(
            cert_pem.as_bytes().to_vec(),
            key_pem.as_bytes().to_vec(),
        )
        .await?;

        println!(
            "[http_config] http config stub on 0.0.0.0:{} (https), log -> captures/http_config.log",
            port
        );
        axum_server::bind_rustls(addr, rustls_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    } else {
        println!(
            "[http_config] http config stub on 0.0.0.0:{}, log -> captures/http_config.log",
            port
        );
        axum_server::bind(addr)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await?;
    }

    Ok(())
}

fn get_or_generate_cert() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    if Path::new(CERT_PATH).exists() && Path::new(KEY_PATH).exists() {
        if let (Ok(cert), Ok(key)) = (fs::read_to_string(CERT_PATH), fs::read_to_string(KEY_PATH)) {
            return Ok((cert, key));
        }
    }

    // Generate self-signed TLS cert for APSEODESK172, localhost, 127.0.0.1
    let subject_alt_names = vec![
        "APSEODESK172".to_string(),
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ];
    let cert = rcgen::generate_simple_self_signed(subject_alt_names)?;
    let cert_pem = cert.cert.pem();
    let key_pem = cert.key_pair.serialize_pem();

    if let Some(parent) = Path::new(CERT_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(CERT_PATH, &cert_pem);
    let _ = fs::write(KEY_PATH, &key_pem);

    Ok((cert_pem, key_pem))
}
