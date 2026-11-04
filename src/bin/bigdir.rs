use fo3_server::tools::bigdir::{entries, read_entry};
use std::env;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 4 {
        let big = &args[1];
        let name = &args[2];
        let out_path = &args[3];

        let data = read_entry(big, name).map_err(|e| std::io::Error::other(e))?;
        if let Some(parent) = Path::new(out_path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(out_path, &data)?;
        println!("{} bytes -> {}", data.len(), out_path);
        return Ok(());
    }

    if args.len() == 2 || args.len() == 3 {
        let big = &args[1];
        let sub = if args.len() == 3 {
            args[2].to_lowercase()
        } else {
            String::new()
        };

        let all = entries(big).map_err(|e| std::io::Error::other(e))?;
        for e in all {
            if sub.is_empty() || e.name.to_lowercase().contains(&sub) {
                println!(
                    "{:#010x} {:>10} {:>10} {}",
                    e.offset, e.size, e.usize_val, e.name
                );
            }
        }
        return Ok(());
    }

    eprintln!(
        "Usage:\n  bigdir <file.big> [substring]          list entries\n  bigdir <file.big> <name> <out_file>    extract entry"
    );
    std::process::exit(1);
}
