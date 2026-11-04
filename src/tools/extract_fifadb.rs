use crate::tools::refpack::unrefpack;
use cipher::{BlockDecryptMut, KeyInit};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const DEFAULT_OFFSET: u64 = 0xEF9F80;
pub const DEFAULT_SIZE: usize = 0x25B4A35;
pub const BLOWFISH_KEY: &[u8] = b"ef4ccfc3c726d060adc11853";

type BlowfishEcbDec = blowfish::Blowfish;

pub fn extract_fifadb(
    data0_big_path: &Path,
    output_sqlite_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut file = File::open(data0_big_path)?;
    file.seek(SeekFrom::Start(DEFAULT_OFFSET))?;
    let mut packed = vec![0u8; DEFAULT_SIZE];
    file.read_exact(&mut packed)?;

    let mut uncompressed = unrefpack(&packed).map_err(|e| std::io::Error::other(e))?;

    // Page-by-page Blowfish-ECB decryption (1024-byte pages)
    // EA encrypts in 32-bit little-endian words
    let cipher = BlowfishEcbDec::new_from_slice(BLOWFISH_KEY)
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    for page in uncompressed.chunks_exact_mut(1024) {
        for chunk in page.chunks_exact_mut(8) {
            // Swap 32-bit LE words for EA's endianness nuance before decrypt
            chunk.swap(0, 3);
            chunk.swap(1, 2);
            chunk.swap(4, 7);
            chunk.swap(5, 6);

            let mut c = cipher.clone();
            c.decrypt_block_mut(chunk.into());

            chunk.swap(0, 3);
            chunk.swap(1, 2);
            chunk.swap(4, 7);
            chunk.swap(5, 6);
        }
    }

    // Rewrite the first 16 bytes: "SQLite format 3\0"
    if uncompressed.len() >= 16 {
        uncompressed[..16].copy_from_slice(b"SQLite format 3\0");
    }

    if let Some(parent) = output_sqlite_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_sqlite_path, uncompressed)?;
    Ok(())
}
