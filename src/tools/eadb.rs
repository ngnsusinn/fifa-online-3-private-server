use byteorder::{ByteOrder, LittleEndian};
use std::collections::HashMap;

pub fn crc(data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFFFFFF;
    for &b in data {
        c ^= (b as u32) << 24;
        for _ in 0..8 {
            if (c & 0x80000000) != 0 {
                c = ((c << 1) ^ 0x04C11DB7) & 0xFFFFFFFF;
            } else {
                c = (c << 1) & 0xFFFFFFFF;
            }
        }
    }
    c
}

#[derive(Debug, Clone)]
pub struct FieldDesc {
    pub field_type: u32,
    pub bit_offset: u32,
    pub short_name: String,
    pub bit_depth: u32,
}

#[derive(Debug, Clone)]
pub struct EaTable {
    pub header: Vec<u8>,
    pub record_size: usize,
    pub fields: HashMap<String, FieldDesc>,
    pub rows: Vec<Vec<u8>>,
}

impl EaTable {
    pub fn parse(raw: &[u8]) -> Result<Self, String> {
        if raw.len() < 0x20 {
            return Err("table buffer too short for header".into());
        }
        let record_size = LittleEndian::read_u32(&raw[4..8]) as usize;
        let nrec = LittleEndian::read_u16(&raw[0x10..0x12]) as usize;
        let field_count = raw[0x18] as usize;

        let mut p = 0x24;
        let mut fields = HashMap::new();
        for _ in 0..field_count {
            if p + 16 > raw.len() {
                return Err("table buffer too short for field descriptors".into());
            }
            let field_type = LittleEndian::read_u32(&raw[p..p + 4]);
            let bit_offset = LittleEndian::read_u32(&raw[p + 4..p + 8]);
            let short_name_raw = &raw[p + 8..p + 12];
            let bit_depth = LittleEndian::read_u32(&raw[p + 12..p + 16]);
            let sn = String::from_utf8_lossy(short_name_raw)
                .trim_end_matches('\0')
                .to_string();

            fields.insert(
                sn.clone(),
                FieldDesc {
                    field_type,
                    bit_offset,
                    short_name: sn,
                    bit_depth,
                },
            );
            p += 16;
        }

        let mut rows = Vec::with_capacity(nrec);
        for i in 0..nrec {
            let row_start = p + i * record_size;
            let row_end = row_start + record_size;
            if row_end > raw.len() {
                break;
            }
            rows.push(raw[row_start..row_end].to_vec());
        }

        Ok(EaTable {
            header: raw[..0x20].to_vec(),
            record_size,
            fields,
            rows,
        })
    }

    pub fn get_string(&self, row: &[u8], short_name: &str) -> Option<String> {
        let desc = self.fields.get(short_name)?;
        if desc.field_type != 0 {
            return None;
        }
        let start = (desc.bit_offset / 8) as usize;
        let end = ((desc.bit_offset + desc.bit_depth) / 8) as usize;
        if end > row.len() {
            return None;
        }
        let slice = &row[start..end];
        let null_pos = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        Some(String::from_utf8_lossy(&slice[..null_pos]).into_owned())
    }

    pub fn get_int(&self, row: &[u8], short_name: &str) -> Option<u64> {
        let desc = self.fields.get(short_name)?;
        if desc.field_type == 0 {
            return None;
        }
        let bo = desc.bit_offset as usize;
        let dep = desc.bit_depth as usize;
        let mut val: u128 = 0;
        for (i, &b) in row.iter().enumerate() {
            val |= (b as u128) << (i * 8);
        }
        let mask = if dep >= 128 {
            !0u128
        } else {
            (1u128 << dep) - 1
        };
        let extracted = (val >> bo) & mask;
        Some(extracted as u64)
    }
}
