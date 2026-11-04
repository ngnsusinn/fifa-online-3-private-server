use std::collections::HashMap;

pub fn unrefpack(src: &[u8]) -> Result<Vec<u8>, String> {
    if src.len() < 2 {
        return Err("data too short for refpack".into());
    }
    let flags = src[0];
    if src[1] != 0xFB {
        return Err("not refpack (signature != 0xFB)".into());
    }
    let n = if (flags & 0x80) != 0 { 4 } else { 3 };
    let mut p = 2 + (if (flags & 0x01) != 0 { n } else { 0 });
    if p + n > src.len() {
        return Err("buffer too short for refpack usize".into());
    }
    let mut usize_val = 0usize;
    for &b in &src[p..p + n] {
        usize_val = (usize_val << 8) | (b as usize);
    }
    p += n;

    let mut out = Vec::with_capacity(usize_val);
    while p < src.len() {
        let b0 = src[p] as usize;
        let plain: usize;
        let length: usize;
        let off: usize;

        if b0 < 0x80 {
            if p + 1 >= src.len() {
                break;
            }
            let b1 = src[p + 1] as usize;
            p += 2;
            plain = b0 & 3;
            length = ((b0 & 0x1C) >> 2) + 3;
            off = ((b0 & 0x60) << 3) + b1 + 1;
        } else if b0 < 0xC0 {
            if p + 2 >= src.len() {
                break;
            }
            let b1 = src[p + 1] as usize;
            let b2 = src[p + 2] as usize;
            p += 3;
            plain = b1 >> 6;
            length = (b0 & 0x3F) + 4;
            off = ((b1 & 0x3F) << 8) + b2 + 1;
        } else if b0 < 0xE0 {
            if p + 3 >= src.len() {
                break;
            }
            let b1 = src[p + 1] as usize;
            let b2 = src[p + 2] as usize;
            let b3 = src[p + 3] as usize;
            p += 4;
            plain = b0 & 3;
            length = ((b0 & 0x0C) << 6) + b3 + 5;
            off = ((b0 & 0x10) << 12) + (b1 << 8) + b2 + 1;
        } else if b0 < 0xFC {
            p += 1;
            plain = ((b0 & 0x1F) << 2) + 4;
            length = 0;
            off = 0;
        } else {
            p += 1;
            let rem = b0 & 3;
            if p + rem <= src.len() {
                out.extend_from_slice(&src[p..p + rem]);
            }
            break;
        }

        if p + plain > src.len() {
            return Err("plain run exceeds input buffer".into());
        }
        out.extend_from_slice(&src[p..p + plain]);
        p += plain;

        if length > 0 {
            if off > out.len() {
                return Err("back-reference offset before start of buffer".into());
            }
            let start = out.len() - off;
            for i in 0..length {
                let byte = out[start + i];
                out.push(byte);
            }
        }
    }

    if out.len() != usize_val {
        return Err(format!(
            "decompressed size mismatch: got {}, expected {}",
            out.len(),
            usize_val
        ));
    }
    Ok(out)
}

pub fn refpack(data: &[u8]) -> Vec<u8> {
    let n = data.len();
    let size = if n >= 1 << 24 { 4 } else { 3 };
    let mut out = Vec::new();
    out.push(if size == 4 { 0x90 } else { 0x10 });
    out.push(0xFB);

    if size == 4 {
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(((n >> 16) & 0xFF) as u8);
        out.push(((n >> 8) & 0xFF) as u8);
        out.push((n & 0xFF) as u8);
    }

    let mut last = HashMap::<[u8; 4], usize>::new();
    let mut i = 0usize;
    let mut lit = 0usize;

    let flush_literals = |out: &mut Vec<u8>, lit: &mut usize, end: usize| -> usize {
        while end - *lit >= 4 {
            let k = (end - *lit).min(112) & !3;
            out.push(0xE0 + (k / 4 - 1) as u8);
            out.extend_from_slice(&data[*lit..*lit + k]);
            *lit += k;
        }
        end - *lit
    };

    while i + 4 <= n {
        let key = [data[i], data[i + 1], data[i + 2], data[i + 3]];
        let j_opt = last.insert(key, i);
        if j_opt.is_none() || i - j_opt.unwrap() > 0x20000 {
            i += 1;
            continue;
        }
        let j = j_opt.unwrap();
        let mut length = 4usize;
        let cap = (n - i).min(1028);
        while length < cap && data[j + length] == data[i + length] {
            length += 1;
        }
        let o = i - j - 1;
        if length < 5 && o >= 0x4000 {
            i += 1;
            continue;
        }

        if length <= 10 && o < 0x400 {
            let plain = flush_literals(&mut out, &mut lit, i);
            out.push((((o >> 8) << 5) | ((length - 3) << 2) | plain) as u8);
            out.push((o & 0xFF) as u8);
        } else if length <= 67 && o < 0x4000 {
            let plain = flush_literals(&mut out, &mut lit, i);
            out.push((0x80 | (length - 4)) as u8);
            out.push(((plain << 6) | (o >> 8)) as u8);
            out.push((o & 0xFF) as u8);
        } else {
            let plain = flush_literals(&mut out, &mut lit, i);
            out.push((0xC0 | ((o >> 16) << 4) | (((length - 5) >> 8) << 2) | plain) as u8);
            out.push(((o >> 8) & 0xFF) as u8);
            out.push((o & 0xFF) as u8);
            out.push(((length - 5) & 0xFF) as u8);
        }

        out.extend_from_slice(&data[lit..lit + (i - lit)]);
        let mut k = i + 1;
        while k < i + length && k + 4 <= n {
            last.insert([data[k], data[k + 1], data[k + 2], data[k + 3]], k);
            k += 7;
        }
        i += length;
        lit = i;
    }

    let plain = flush_literals(&mut out, &mut lit, n);
    out.push((0xFC + plain) as u8);
    out.extend_from_slice(&data[lit..]);
    out
}
