use libbladerf_rs::bladerf1::{MetadataHeader, SampleFormat};

#[test]
fn metadata_wire_fields_are_little_endian_and_allow_unaligned_input() {
    let bytes = [
        0xFF, 0x34, 0x12, 0x34, 0xAB, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x78, 0x56,
        0x34, 0x12, 0xFF,
    ];
    let header = MetadataHeader::from_bytes(&bytes[1..]).unwrap();
    assert_eq!(header.packet_length(), 0x1234);
    assert_eq!(header.meta_version(), 0x34);
    assert_eq!(header.stream_flags(), 0xAB);
    assert_eq!(header.timestamp(), 0x0102_0304_0506_0708);
    assert_eq!(header.meta_flags(), 0x1234_5678);
    for len in 0..16 {
        assert!(MetadataHeader::from_bytes(&bytes[..len]).is_none());
    }
}

fn pack_i16(value: i16) -> [u8; 2] {
    value.to_le_bytes()
}

#[test]
fn pack_unpack_roundtrip() {
    let samples: [i16; 8] = [0, -1, 2_047, -2048, 1, -100, 0x07FF, -2048i16];
    let num_samples = samples.len();
    let mut src = vec![0u8; 4 * num_samples];
    let mut packed = vec![0u8; 3 * num_samples];
    let mut unpacked = vec![0u8; 4 * num_samples];
    for (i, &s) in samples.iter().enumerate() {
        src[2 * i..2 * i + 2].copy_from_slice(&pack_i16(s));
    }
    SampleFormat::pack_sc16q11_packed(&src, &mut packed, num_samples).unwrap();
    SampleFormat::unpack_sc16q11_packed(&packed, &mut unpacked, num_samples).unwrap();
    for (i, &orig) in samples.iter().enumerate() {
        let got = i16::from_le_bytes([unpacked[2 * i], unpacked[2 * i + 1]]);
        assert_eq!(
            got, orig,
            "Sample {i}: expected {orig:#06x}, got {got:#06x}"
        );
    }
}

#[test]
fn pack_unpack_roundtrip_negative() {
    let samples: [i16; 4] = [-1, -1, -2048, -2048];
    let num_samples = samples.len();
    let mut src = vec![0u8; 4 * num_samples];
    let mut packed = vec![0u8; 3 * num_samples];
    let mut unpacked = vec![0u8; 4 * num_samples];
    for (i, &s) in samples.iter().enumerate() {
        src[2 * i..2 * i + 2].copy_from_slice(&pack_i16(s));
    }
    SampleFormat::pack_sc16q11_packed(&src, &mut packed, num_samples).unwrap();
    SampleFormat::unpack_sc16q11_packed(&packed, &mut unpacked, num_samples).unwrap();
    for (i, &orig) in samples.iter().enumerate() {
        let got = i16::from_le_bytes([unpacked[2 * i], unpacked[2 * i + 1]]);
        assert_eq!(got, orig, "Sample {i}: expected {orig}, got {got}");
    }
}
