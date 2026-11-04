use crate::tools::refpack::unrefpack;
use byteorder::{BigEndian, ByteOrder};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub fn nibswap(x: u32) -> u32 {
    ((x << 4) & 0xF0F0F0F0) | ((x >> 4) & 0x0F0F0F0F)
}

pub fn dec(x: u32) -> u32 {
    !(nibswap(x) ^ 0x2938FA72)
}

pub fn enc(v: u32) -> u32 {
    nibswap(!v ^ 0x2938FA72)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigEntry {
    pub name: String,
    pub offset: u32,
    pub size: u32,
    pub usize_val: u32,
}

pub fn table(path: &str) -> Result<Vec<u8>, String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut head = [0u8; 16];
    f.read_exact(&mut head).map_err(|e| e.to_string())?;
    if &head[..4] != b"BIg4" {
        return Err("not BIg4 archive".into());
    }
    let header_size = dec(BigEndian::read_u32(&head[12..16])) as usize;
    f.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; header_size];
    f.read_exact(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

pub fn entries(path: &str) -> Result<Vec<BigEntry>, String> {
    let t = table(path)?;
    if t.len() < 16 {
        return Err("table buffer too short".into());
    }
    let count = dec(BigEndian::read_u32(&t[8..12])) as usize;
    let mut p = 0x11;
    let mut result = Vec::with_capacity(count);

    for _ in 0..count {
        if p + 16 > t.len() {
            break;
        }
        let off = dec(BigEndian::read_u32(&t[p..p + 4]));
        let size = dec(BigEndian::read_u32(&t[p + 4..p + 8]));
        let usize_val = dec(BigEndian::read_u32(&t[p + 8..p + 12]));
        let nl = dec(BigEndian::read_u32(&t[p + 12..p + 16])) as usize;

        if p + 16 + nl > t.len() {
            break;
        }
        let mut k = (size & 0xFF) as u8;
        let mut name_bytes = Vec::with_capacity(nl);
        for &c in &t[p + 16..p + 16 + nl] {
            let d = ((c << 4) | (c >> 4)) as u8;
            name_bytes.push(d ^ k);
            let term = if d != 0 { d } else { (size & 0xFF) as u8 };
            k = term.wrapping_sub(0x66);
        }
        let name = String::from_utf8_lossy(&name_bytes).into_owned();
        result.push(BigEntry {
            name,
            offset: off,
            size,
            usize_val,
        });
        p += 0x11 + nl;
    }
    Ok(result)
}

pub fn read_entry(path: &str, name: &str) -> Result<Vec<u8>, String> {
    let all = entries(path)?;
    let entry = all
        .into_iter()
        .find(|e| e.name == name)
        .ok_or_else(|| format!("entry not found: {}", name))?;

    let mut f = File::open(path).map_err(|e| e.to_string())?;
    f.seek(SeekFrom::Start(entry.offset as u64))
        .map_err(|e| e.to_string())?;
    let mut data = vec![0u8; entry.size as usize];
    f.read_exact(&mut data).map_err(|e| e.to_string())?;

    if entry.usize_val != 0 && entry.usize_val != entry.size {
        unrefpack(&data)
    } else {
        Ok(data)
    }
}
