use fo3_server::blaze::{
    build_frame, frames_and_rest, parse_header,
    tdf::{
        decode_tag, decode_tdf, decode_varint, enc_blob, enc_field, enc_float, enc_int,
        enc_string, enc_struct, encode_tag, encode_varint, TdfValue, TDF_LIST,
    },
    MSG_MESSAGE, MSG_REPLY,
};

#[test]
fn test_blaze_framing() {
    let b1 = build_frame(0x0005, 0x0001, MSG_MESSAGE, 1, b"\xde\xad\xbe\xef", 0);
    let b2 = build_frame(0x0001, 0x0002, MSG_REPLY, 7, b"", 0);
    let partial = b"\x00\x10\x00";

    let mut buf = Vec::new();
    buf.extend_from_slice(&b1);
    buf.extend_from_slice(&b2);
    buf.extend_from_slice(partial);

    let (frames, rest) = frames_and_rest(&buf);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].0.component, 0x0005);
    assert_eq!(frames[0].0.command, 0x0001);
    assert_eq!(frames[0].0.msgtype_name(), "MESSAGE");
    assert_eq!(frames[0].1, b"\xde\xad\xbe\xef");

    assert_eq!(frames[1].0.msgtype_name(), "REPLY");
    assert_eq!(frames[1].0.msgid, 7);
    assert_eq!(frames[1].1, b"");
    assert_eq!(rest, partial);

    let (frames2, rest2) = frames_and_rest(b"\x00\x04");
    assert!(frames2.is_empty());
    assert_eq!(rest2, b"\x00\x04");

    let hdr = parse_header(&build_frame(9, 1, MSG_REPLY, 0x53ABC, b"", 0)).unwrap();
    assert_eq!(hdr.msgid, 0x53ABC);
    assert_eq!(hdr.msgtype_name(), "REPLY");
}

#[test]
fn test_tag_packing() {
    assert_eq!(encode_tag("T"), [0xd0, 0x00, 0x00]);
    for tag in ["TEST", "UID", "SVID", "PORT", "A1B2", "GID"] {
        let enc = encode_tag(tag);
        assert_eq!(decode_tag(enc[0], enc[1], enc[2]), tag);
    }
    let ip_tag = encode_tag("IP  ");
    assert_eq!(decode_tag(ip_tag[0], ip_tag[1], ip_tag[2]), "IP");
}

#[test]
fn test_varint_round_trip() {
    for n in [0, 1, 0x3f, 0x40, 1000, 1 << 20, 1 << 40] {
        let enc = encode_varint(n);
        let mut off = 0;
        let dec = decode_varint(&enc, &mut off).unwrap();
        assert_eq!(dec, n);
    }
}

#[test]
fn test_tdf_round_trip() {
    let mut body = Vec::new();
    body.extend_from_slice(&enc_int("TEST", 1000));
    body.extend_from_slice(&enc_string("NAME", "hi"));
    body.extend_from_slice(&enc_blob("BLOB", b"\x01\x02\x03"));
    body.extend_from_slice(&enc_struct("USER", &enc_int("UID", 12345)));
    body.extend_from_slice(&enc_float("RATE", 1.5));

    let fields = decode_tdf(&body);
    let mut map = std::collections::HashMap::new();
    for (k, v) in fields {
        map.insert(k, v);
    }

    assert_eq!(map.get("TEST").unwrap(), &TdfValue::Int(1000));
    assert_eq!(map.get("NAME").unwrap(), &TdfValue::String("hi".into()));
    assert_eq!(map.get("BLOB").unwrap(), &TdfValue::Blob(vec![1, 2, 3]));
    let user_struct = map.get("USER").unwrap().as_struct().unwrap();
    assert_eq!(user_struct[0].0, "UID");
    assert_eq!(user_struct[0].1, TdfValue::Int(12345));
    assert_eq!(map.get("RATE").unwrap(), &TdfValue::Float(1.5));

    // Unknown/unvalidated type should degrade to Undecoded
    let bad = decode_tdf(&enc_field("LIST", TDF_LIST, b"\x99\x99"));
    assert_eq!(bad[0].0, "LIST");
    match &bad[0].1 {
        TdfValue::Undecoded { type_id, rest } => {
            assert_eq!(*type_id, TDF_LIST);
            assert_eq!(rest, b"\x99\x99");
        }
        _ => panic!("Expected Undecoded"),
    }
}

#[test]
fn test_full_frame_round_trip() {
    let mut inner = Vec::new();
    inner.extend_from_slice(&enc_string("HOST", "127.0.0.1"));
    inner.extend_from_slice(&enc_int("PORT", 28000));

    let mut body = Vec::new();
    body.extend_from_slice(&enc_struct("SINS", &inner));
    body.extend_from_slice(&enc_int("SVID", 7));

    let frame = build_frame(5, 1, MSG_REPLY, 42, &body, 0);
    let (frames, rest) = frames_and_rest(&frame);
    assert_eq!(frames.len(), 1);
    assert!(rest.is_empty());

    let (hdr, payload) = &frames[0];
    assert_eq!(hdr.component, 5);
    assert_eq!(hdr.command, 1);
    assert_eq!(hdr.msgtype_name(), "REPLY");
    assert_eq!(hdr.msgid, 42);

    let decoded = decode_tdf(payload);
    let map: std::collections::HashMap<_, _> = decoded.into_iter().collect();
    let sins = map.get("SINS").unwrap().as_struct().unwrap();
    let sins_map: std::collections::HashMap<_, _> = sins.iter().cloned().collect();

    assert_eq!(sins_map.get("HOST").unwrap(), &TdfValue::String("127.0.0.1".into()));
    assert_eq!(sins_map.get("PORT").unwrap(), &TdfValue::Int(28000));
    assert_eq!(map.get("SVID").unwrap(), &TdfValue::Int(7));
}
