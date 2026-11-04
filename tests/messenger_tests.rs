use byteorder::{ByteOrder, LittleEndian};
use fo3_server::messenger::{decode, encode};
use serde_json::json;

#[test]
fn test_messenger_encode_decode() {
    let m = json!({"cmd": "auth", "sessionid": ""});
    let f = encode(&m).unwrap();
    let ln = LittleEndian::read_u16(&f[0..2]) as usize;
    let ty = LittleEndian::read_u16(&f[2..4]);

    assert_eq!(ty, 2);
    assert_eq!(ln, f.len() - 4);
    assert_eq!(ln % 8, 0);

    let decoded = decode(&f[4..]).unwrap();
    assert_eq!(decoded, m);

    // Real client's captured first frame body
    let real_hex = "8b412a58950cd8431cb7b2e7473b61a8534a576ac50317310d047b5400622951";
    let real_bytes: Vec<u8> = (0..real_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&real_hex[i..i + 2], 16).unwrap())
        .collect();
    let real_decoded = decode(&real_bytes).unwrap();
    assert_eq!(real_decoded, m);
}
