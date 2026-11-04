use fo3_server::tools::refpack::{refpack, unrefpack};
use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

fn parse_hex_or_dec(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        u64::from_str_radix(&s[2..], 16).map_err(|e| e.to_string())
    } else {
        u64::from_str_radix(s, 16)
            .or_else(|_| s.parse::<u64>())
            .map_err(|e| e.to_string())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--selftest") {
        let sample = b"FIFA Online 3 RefPack Compression and Decompression Test String 1234567890!";
        let packed = refpack(sample);
        let unpacked = unrefpack(&packed).map_err(|e| std::io::Error::other(e))?;
        assert_eq!(sample.as_slice(), unpacked.as_slice());
        println!("refpack selftest OK");
        return Ok(());
    }

    if args.len() == 4 && args[1] == "-c" {
        let input = std::fs::read(&args[2])?;
        let packed = refpack(&input);
        std::fs::write(&args[3], packed)?;
        println!("Compressed {} bytes -> {}", input.len(), args[3]);
        return Ok(());
    }

    if args.len() == 4 && args[1] == "-d" {
        let input = std::fs::read(&args[2])?;
        let unpacked = unrefpack(&input).map_err(|e| std::io::Error::other(e))?;
        std::fs::write(&args[3], unpacked)?;
        println!("Decompressed {} bytes -> {}", input.len(), args[3]);
        return Ok(());
    }

    if args.len() == 5 {
        let big = &args[1];
        let off = parse_hex_or_dec(&args[2])?;
        let size = parse_hex_or_dec(&args[3])? as usize;
        let dst = &args[4];

        let mut f = File::open(big)?;
        f.seek(SeekFrom::Start(off))?;
        let mut data = vec![0u8; size];
        f.read_exact(&mut data)?;

        let res = unrefpack(&data).map_err(|e| std::io::Error::other(e))?;
        if let Some(parent) = Path::new(dst).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(dst, &res)?;
        let preview = if res.len() > 16 { &res[..16] } else { &res[..] };
        println!("{} bytes -> {:?}: {:?}", res.len(), dst, preview);
        return Ok(());
    }

    eprintln!(
        "Usage:\n  refpack <big> <off_hex> <csize_hex> <out>\n  refpack -c <in> <out>\n  refpack -d <in> <out>\n  refpack --selftest"
    );
    std::process::exit(1);
}
