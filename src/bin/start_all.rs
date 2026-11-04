use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn find_binary(name: &str) -> PathBuf {
    if let Ok(current_exe) = env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(format!("{}{}", name, env::consts::EXE_SUFFIX));
            if candidate.exists() {
                return candidate;
            }
        }
    }
    let target_debug = PathBuf::from("target")
        .join("debug")
        .join(format!("{}{}", name, env::consts::EXE_SUFFIX));
    if target_debug.exists() {
        return target_debug;
    }
    PathBuf::from(name)
}

fn pump_stream<R: io::Read + Send + 'static>(name: String, reader: R) {
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    while let Ok(n) = buf_reader.read_line(&mut line) {
        if n == 0 {
            break;
        }
        print!("[{}] {}", name, line);
        let _ = io::stdout().flush();
        line.clear();
    }
}

fn spawn_service(name: &'static str, args: &[&str]) -> io::Result<Child> {
    let bin_path = find_binary(name);
    let mut cmd = Command::new(&bin_path);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        io::Error::other(format!(
            "Failed to spawn {} ({:?}): {}",
            name, bin_path, e
        ))
    })?;

    if let Some(stdout) = child.stdout.take() {
        let name_owned = name.to_string();
        std::thread::spawn(move || pump_stream(name_owned, stdout));
    }
    if let Some(stderr) = child.stderr.take() {
        let name_owned = name.to_string();
        std::thread::spawn(move || pump_stream(name_owned, stderr));
    }

    Ok(child)
}

fn shutdown_procs(procs: &mut [(String, Child)]) {
    for (_, p) in procs.iter_mut() {
        let _ = p.kill();
    }
    for (_, p) in procs.iter_mut() {
        let _ = p.wait();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let selftest = args.iter().any(|a| a == "--selftest");

    let http_args: Vec<&str> = if let Ok(port) = env::var("FO3_HTTP_PORT") {
        vec![Box::leak(port.into_boxed_str())]
    } else {
        vec!["443", "--tls"]
    };

    let mut procs = Vec::new();

    match spawn_service("http_config", &http_args) {
        Ok(c) => procs.push(("http_config".to_string(), c)),
        Err(e) => eprintln!("[start_all] failed to start http_config: {}", e),
    }

    match spawn_service("messenger", &[]) {
        Ok(c) => procs.push(("messenger".to_string(), c)),
        Err(e) => eprintln!("[start_all] failed to start messenger: {}", e),
    }

    match spawn_service("server", &[]) {
        Ok(c) => procs.push(("server".to_string(), c)),
        Err(e) => eprintln!("[start_all] failed to start server: {}", e),
    }

    println!(
        "[start_all] http_config (:443 https, admin :8088), messenger (:28002), server (:28000/:28001) up. Ctrl-C to stop."
    );

    if selftest {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let mut all_alive = true;
        for (name, p) in procs.iter_mut() {
            match p.try_wait() {
                Ok(Some(status)) => {
                    eprintln!("[start_all] service {} exited prematurely with {}", name, status);
                    all_alive = false;
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("[start_all] error checking {}: {}", name, e);
                    all_alive = false;
                }
            }
        }
        shutdown_procs(&mut procs);
        if all_alive {
            println!("[start_all] selftest OK");
            std::process::exit(0);
        } else {
            eprintln!("[start_all] selftest FAILED");
            std::process::exit(1);
        }
    }

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n[start_all] received Ctrl-C, stopping...");
        }
        _ = async {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                for (name, p) in procs.iter_mut() {
                    if let Ok(Some(status)) = p.try_wait() {
                        eprintln!("[start_all] service {} exited with {}", name, status);
                        return;
                    }
                }
            }
        } => {}
    }

    shutdown_procs(&mut procs);
    println!("[start_all] stopped.");
    Ok(())
}
