use byteorder::{BigEndian, ByteOrder};

pub const TDF_INT: u8 = 0x00;
pub const TDF_STRING: u8 = 0x01;
pub const TDF_BLOB: u8 = 0x02;
pub const TDF_STRUCT: u8 = 0x03;
pub const TDF_LIST: u8 = 0x04;
pub const TDF_MAP: u8 = 0x05;
pub const TDF_UNION: u8 = 0x06;
pub const TDF_INTLIST: u8 = 0x07;
pub const TDF_OBJTYPE: u8 = 0x08;
pub const TDF_OBJID: u8 = 0x09;
pub const TDF_FLOAT: u8 = 0x0A;
pub const TDF_GENERIC: u8 = 0x0C;

pub fn encode_tag(label: &str) -> [u8; 3] {
    let t = label.as_bytes();
    let mut out = [0u8; 3];
    let n = t.len().min(4);
    if n > 0 {
        out[0] |= (t[0] & 0x40) << 1;
        out[0] |= (t[0] & 0x10) << 2;
        out[0] |= (t[0] & 0x0F) << 2;
    }
    if n > 1 {
        out[0] |= (t[1] & 0x40) >> 5;
        out[0] |= (t[1] & 0x10) >> 4;
        out[1] |= (t[1] & 0x0F) << 4;
    }
    if n > 2 {
        out[1] |= (t[2] & 0x40) >> 3;
        out[1] |= (t[2] & 0x10) >> 2;
        out[1] |= (t[2] & 0x0C) >> 2;
        out[2] |= (t[2] & 0x03) << 6;
    }
    if n > 3 {
        out[2] |= (t[3] & 0x40) >> 1;
        out[2] |= t[3] & 0x1F;
    }
    out
}

pub fn decode_tag(b0: u8, b1: u8, b2: u8) -> String {
    let dec = |m: u8, c: u8| -> u8 {
        if (m | c) == 0 {
            0
        } else if (m & 0x40) == 0 {
            0x30 | c
        } else {
            m | c
        }
    };
    let o = [
        dec((b0 & 0x80) >> 1, (b0 & 0x7C) >> 2),
        dec((b0 & 2) << 5, ((b0 & 1) << 4) | ((b1 & 0xF0) >> 4)),
        dec((b1 & 8) << 3, ((b1 & 7) << 2) | ((b2 & 0xC0) >> 6)),
        dec((b2 & 0x20) << 1, b2 & 0x1F),
    ];
    o.into_iter()
        .filter(|&x| x != 0)
        .map(|x| x as char)
        .collect()
}

pub fn encode_varint(mut n: u64) -> Vec<u8> {
    if n < 0x40 {
        return vec![n as u8];
    }
    let mut out = vec![0x80 | ((n & 0x3F) as u8)];
    n >>= 6;
    while n >= 0x80 {
        out.push(0x80 | ((n & 0x7F) as u8));
        n >>= 7;
    }
    out.push((n & 0x7F) as u8);
    out
}

pub fn decode_varint(buf: &[u8], off: &mut usize) -> Result<u64, &'static str> {
    if *off >= buf.len() {
        return Err("unexpected end of buffer decoding varint");
    }
    let mut b = buf[*off];
    *off += 1;
    let mut v = (b & 0x3F) as u64;
    let mut shift = 6;
    while (b & 0x80) != 0 {
        if *off >= buf.len() {
            return Err("unexpected end of buffer decoding varint continuation");
        }
        b = buf[*off];
        *off += 1;
        v |= ((b & 0x7F) as u64) << shift;
        shift += 7;
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
pub enum TdfValue {
    Int(u64),
    String(String),
    Blob(Vec<u8>),
    Struct(Vec<(String, TdfValue)>),
    Float(f32),
    Undecoded { type_id: u8, rest: Vec<u8> },
}

impl TdfValue {
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            TdfValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            TdfValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&[(String, TdfValue)]> {
        match self {
            TdfValue::Struct(s) => Some(s.as_slice()),
            _ => None,
        }
    }
}

pub fn enc_field(label: &str, type_id: u8, value_bytes: &[u8]) -> Vec<u8> {
    let tag = encode_tag(label);
    let mut out = Vec::with_capacity(3 + 1 + value_bytes.len());
    out.extend_from_slice(&tag);
    out.push(type_id);
    out.extend_from_slice(value_bytes);
    out
}

pub fn enc_int(label: &str, n: u64) -> Vec<u8> {
    enc_field(label, TDF_INT, &encode_varint(n))
}

pub fn enc_string(label: &str, s: &str) -> Vec<u8> {
    let mut b = s.as_bytes().to_vec();
    b.push(0x00); // null terminated
    let var = encode_varint(b.len() as u64);
    let mut val = Vec::with_capacity(var.len() + b.len());
    val.extend_from_slice(&var);
    val.extend_from_slice(&b);
    enc_field(label, TDF_STRING, &val)
}

pub fn enc_blob(label: &str, data: &[u8]) -> Vec<u8> {
    let var = encode_varint(data.len() as u64);
    let mut val = Vec::with_capacity(var.len() + data.len());
    val.extend_from_slice(&var);
    val.extend_from_slice(data);
    enc_field(label, TDF_BLOB, &val)
}

pub fn enc_struct(label: &str, inner_bytes: &[u8]) -> Vec<u8> {
    let mut val = Vec::with_capacity(inner_bytes.len() + 1);
    val.extend_from_slice(inner_bytes);
    val.push(0x00); // 0x00 struct terminator
    enc_field(label, TDF_STRUCT, &val)
}

pub fn enc_float(label: &str, f: f32) -> Vec<u8> {
    let mut b = [0u8; 4];
    BigEndian::write_f32(&mut b, f);
    enc_field(label, TDF_FLOAT, &b)
}

fn decode_value(buf: &[u8], off: &mut usize, type_id: u8) -> Result<TdfValue, u8> {
    match type_id {
        TDF_INT => match decode_varint(buf, off) {
            Ok(v) => Ok(TdfValue::Int(v)),
            Err(_) => Err(type_id),
        },
        TDF_STRING => {
            let ln = match decode_varint(buf, off) {
                Ok(l) => l as usize,
                Err(_) => return Err(type_id),
            };
            if *off + ln > buf.len() {
                return Err(type_id);
            }
            let raw = &buf[*off..*off + ln];
            *off += ln;
            let trimmed = if raw.ends_with(&[0]) {
                &raw[..raw.len() - 1]
            } else {
                raw
            };
            let s = String::from_utf8_lossy(trimmed).into_owned();
            Ok(TdfValue::String(s))
        }
        TDF_BLOB => {
            let ln = match decode_varint(buf, off) {
                Ok(l) => l as usize,
                Err(_) => return Err(type_id),
            };
            if *off + ln > buf.len() {
                return Err(type_id);
            }
            let data = buf[*off..*off + ln].to_vec();
            *off += ln;
            Ok(TdfValue::Blob(data))
        }
        TDF_STRUCT => {
            let fields = decode_struct(buf, off);
            Ok(TdfValue::Struct(fields))
        }
        TDF_FLOAT => {
            if *off + 4 > buf.len() {
                return Err(type_id);
            }
            let f = BigEndian::read_f32(&buf[*off..*off + 4]);
            *off += 4;
            Ok(TdfValue::Float(f))
        }
        unsupported => Err(unsupported),
    }
}

fn decode_struct(buf: &[u8], off: &mut usize) -> Vec<(String, TdfValue)> {
    let mut fields = Vec::new();
    let n = buf.len();
    while *off < n {
        if buf[*off] == 0x00 {
            *off += 1;
            return fields;
        }
        if *off + 4 > n {
            break;
        }
        let tag = decode_tag(buf[*off], buf[*off + 1], buf[*off + 2]);
        let type_id = buf[*off + 3];
        *off += 4;
        match decode_value(buf, off, type_id) {
            Ok(val) => fields.push((tag, val)),
            Err(err_type) => {
                let rest = buf[*off..].to_vec();
                *off = n;
                fields.push((
                    tag,
                    TdfValue::Undecoded {
                        type_id: err_type,
                        rest,
                    },
                ));
                return fields;
            }
        }
    }
    fields
}

pub fn decode_tdf(buf: &[u8]) -> Vec<(String, TdfValue)> {
    let mut off = 0;
    decode_struct(buf, &mut off)
}
