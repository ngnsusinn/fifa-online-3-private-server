use crate::blaze::{describe, frames_and_rest, tdf::*};

fn walk_fields<'a>(fields: &'a [(String, TdfValue)], out: &mut Vec<(&'a str, &'a TdfValue)>) {
    for (tag, val) in fields {
        out.push((tag.as_str(), val));
        if let TdfValue::Struct(inner) = val {
            walk_fields(inner, out);
        }
    }
}

fn fmt_fields(fields: &[(String, TdfValue)], indent: usize) -> Vec<String> {
    let mut out = Vec::new();
    let pad = " ".repeat(indent);
    for (tag, val) in fields {
        match val {
            TdfValue::Struct(inner) => {
                out.push(format!("{}{}: (struct)", pad, tag));
                out.extend(fmt_fields(inner, indent + 4));
            }
            TdfValue::Undecoded { type_id, rest } => {
                let preview = if rest.len() > 32 {
                    format!("{}...", crate::blaze::hex::encode(&rest[..32]))
                } else {
                    crate::blaze::hex::encode(rest)
                };
                out.push(format!(
                    "{}{}: !!<Undecoded type=0x{:02x} rest={}>",
                    pad, tag, type_id, preview
                ));
            }
            TdfValue::String(s) => {
                out.push(format!("{}{}: {:?}", pad, tag, s));
            }
            TdfValue::Int(n) => {
                out.push(format!("{}{}: {}", pad, tag, n));
            }
            TdfValue::Float(f) => {
                out.push(format!("{}{}: {}", pad, tag, f));
            }
            TdfValue::Blob(b) => {
                out.push(format!("{}{}: (blob len={})", pad, tag, b.len()));
            }
        }
    }
    out
}

pub fn analyze(data: &[u8]) -> (Vec<String>, usize, bool) {
    let (frames, rest) = frames_and_rest(data);
    let mut out = Vec::new();
    let mut undecoded = false;

    for (i, (hdr, payload)) in frames.iter().enumerate() {
        out.push(format!("frame#{}  {}", i, describe(hdr, payload)));
        if !payload.is_empty() {
            let fields = decode_tdf(payload);
            out.extend(fmt_fields(&fields, 4));
            let mut flat = Vec::new();
            walk_fields(&fields, &mut flat);
            if flat
                .iter()
                .any(|(_, v)| matches!(v, TdfValue::Undecoded { .. }))
            {
                undecoded = true;
            }
        }
    }

    if !rest.is_empty() {
        let hex_slice = if rest.len() > 48 {
            format!("{} ...", crate::blaze::hex::encode(&rest[..48]))
        } else {
            crate::blaze::hex::encode(&rest)
        };
        out.push(format!(
            "** trailing {} undecoded bytes: {}",
            rest.len(),
            hex_slice
        ));
        undecoded = true;
    }

    (out, frames.len(), undecoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blaze::build_frame;

    #[test]
    fn test_analyze_capture_selftest() {
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
    }
}
