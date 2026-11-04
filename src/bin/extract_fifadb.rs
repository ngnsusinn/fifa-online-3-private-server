use cipher::{BlockDecryptMut, KeyInit};
use fo3_server::tools::extract_fifadb::{extract_fifadb, BLOWFISH_KEY};
use std::env;
use std::path::{Path, PathBuf};

fn selftest() {
    type BlowfishEcbDec = blowfish::Blowfish;
    let cipher = BlowfishEcbDec::new_from_slice(BLOWFISH_KEY).expect("cipher init failed");
    let hex_ct = "9c7b6c2239dbbfed";
    let mut block = [0u8; 8];
    for i in 0..8 {
        block[i] = u8::from_str_radix(&hex_ct[i * 2..i * 2 + 2], 16).unwrap();
    }
    // Swap 32-bit LE words for EA's endianness nuance
    block.swap(0, 3);
    block.swap(1, 2);
    block.swap(4, 7);
    block.swap(5, 6);

    let mut c = cipher;
    c.decrypt_block_mut((&mut block).into());

    block.swap(0, 3);
    block.swap(1, 2);
    block.swap(4, 7);
    block.swap(5, 6);

    assert_eq!(block, [0u8; 8]);
    println!("extract_fifadb selftest OK");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--selftest") {
        selftest();
        return Ok(());
    }

    let default_game = env::var("FO3_GAME").unwrap_or_else(|_| r"D:\FO3ID\32795".to_string());
    let target = args.get(1).cloned().unwrap_or(default_game);
    let out_str = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "gamedata/fifadb.sqlite".to_string());

    let target_path = PathBuf::from(&target);
    let big_path = if target_path.is_file() {
        target_path
    } else {
        target_path.join("data").join("data_0.big")
    };

    let out_path = Path::new(&out_str);

    if !big_path.exists() {
        eprintln!(
            "Error: data_0.big not found at {:?}. Provide path as first argument or set FO3_GAME.",
            big_path
        );
        std::process::exit(1);
    }

    println!("Extracting fifadb from {:?} -> {:?}", big_path, out_path);
    extract_fifadb(&big_path, out_path)?;
    println!("Successfully extracted fifadb database to {:?}", out_path);

    Ok(())
}
