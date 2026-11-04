use fo3_server::tools::refpack::{refpack, unrefpack};

#[test]
fn test_refpack_round_trip() {
    let original = b"Hello, EA RefPack compression and decompression! Hello, EA RefPack compression and decompression!";
    let packed = refpack(original);
    assert_eq!(packed[1], 0xFB);
    let unpacked = unrefpack(&packed).unwrap();
    assert_eq!(unpacked, original);
}
