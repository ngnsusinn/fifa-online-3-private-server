use fo3_server::blaze::{build_frame, tdf::*};
use fo3_server::tools::analyze_capture::analyze;
use std::env;
use std::fs;

fn selftest() {
    let mut inner = Vec::new();
    inner.extend_from_slice(&enc_string("HOST", "1.2.3.4"));
    inner.extend_from_slice(&enc_int("PORT", 9));

    let mut body = Vec::new();
    body.extend_from_slice(&enc_struct("SINS", &inner));
    body.extend_from_slice(&enc_int("SVID", 3));

    let mut buf = build_frame(5, 1, 1, 1, &body, 0);
    let frame2 = build_frame(1, 0x0C, 1, 2, &enc_field("LIST", TDF_LIST, b"\x01\x02"), 0);
    buf.extend_from_slice(&frame2);

    let (out, n, undec) = analyze(&buf);
    let text = out.join("\n");

    assert_eq!(n, 2);
    assert!(text.contains("SINS: (struct)"));
    assert!(text.contains("HOST: \"1.2.3.4\""));
    assert!(text.contains("PORT: 9"));
    assert!(text.contains("!!<Undecoded"));
    assert!(undec);

    println!("analyze_capture selftest OK");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--selftest") {
        selftest();
        return Ok(());
    }

    if args.len() < 2 {
        eprintln!("Usage: analyze_capture <captures/*.bin ...> [--selftest]");
        std::process::exit(1);
    }

    for path in args.iter().skip(1) {
        if path.starts_with("--") {
            continue;
        }
        println!("=== {} ===", path);
        match fs::read(path) {
            Ok(data) => {
                let (lines, n, undec) = analyze(&data);
                for ln in lines {
                    println!("{}", ln);
                }
                println!("summary: {} frames, undecoded={}\n", n, undec);
            }
            Err(e) => {
                eprintln!("Failed to read {}: {}\n", path, e);
            }
        }
    }

    Ok(())
}
