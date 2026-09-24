use libbladerf_rs::Error;
use libbladerf_rs::protocol::nios::{NiosPacketError, nios_decode_read, nios_decode_write};
use libbladerf_rs::protocol::nios::{NiosPkt, NiosPktFlags};

#[test]
fn response_requires_matching_family_direction_and_success() {
    let response = [
        0x43, 1, 2, 0, 0x12, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(nios_decode_read::<u8, u32>(&response).unwrap(), 0x1234_5678);
    assert!(matches!(
        nios_decode_read::<u8, u16>(&response),
        Err(Error::NiosPacket(NiosPacketError::MagicMismatch { .. }))
    ));
    assert!(matches!(
        nios_decode_write::<u8, u32>(&response),
        Err(Error::NiosPacket(NiosPacketError::ResponseMismatch))
    ));
    let mut failed = response;
    failed[2] = 0;
    assert!(matches!(
        nios_decode_read::<u8, u32>(&failed),
        Err(Error::NiosPacket(NiosPacketError::ReadFailed))
    ));
    failed[2] = 1;
    assert!(matches!(
        nios_decode_write::<u8, u32>(&failed),
        Err(Error::NiosPacket(NiosPacketError::WriteFailed))
    ));
    failed[2] = 3;
    assert!(nios_decode_write::<u8, u32>(&failed).is_ok());
    for len in 0..16 {
        assert!(nios_decode_read::<u8, u32>(&response[..len]).is_err());
    }
    assert!(nios_decode_read::<u8, u32>(&[0; 17]).is_err());
}

#[test]
fn preparing_a_request_clears_stale_data_and_reserved_fields() {
    let mut bytes = [0xFF; 16];
    NiosPkt::<u8, u8>::new(&mut bytes)
        .unwrap()
        .prepare_read(1, 2);
    assert_eq!(bytes, [0x41, 1, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

const EXPECTED_MAGIC_8X8: u8 = 0x41;

fn make_buf() -> [u8; 16] {
    [0u8; 16]
}

#[test]
fn packet_encode_decode() {
    let mut buf = make_buf();
    NiosPkt::<u8, u8>::new(&mut buf)
        .unwrap()
        .prepare_write(1, 3, 4);
    assert_eq!(buf[0], EXPECTED_MAGIC_8X8);

    let packet = NiosPkt::<u8, u8>::new(&mut buf).unwrap();
    assert_eq!(packet.target(), 1);
}

#[test]
fn packet_from_slice() {
    let mut buf = make_buf();
    let packet = NiosPkt::<u8, u8>::new(&mut buf).unwrap();

    assert_eq!(0x0, packet.target());
    assert_eq!(NiosPktFlags::Read, packet.flags());
    assert_eq!(0x0, packet.addr());
    assert_eq!(0x0, packet.data());
}

#[test]
fn packet8x8_new() {
    let target_id = 0x1;
    let addr = 0x12;
    let data = 0x12;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u8, u8>::new(&mut buf).unwrap();
    packet.prepare_write(target_id, addr, data);
    let packet = NiosPkt::<u8, u8>::new(&mut buf).unwrap();
    assert_eq!(target_id, packet.target());
    assert_eq!(NiosPktFlags::Write, packet.flags());
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}

#[test]
fn packet8x16_new() {
    let addr = 0x12;
    let data = 0x1234;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u8, u16>::new(&mut buf).unwrap();
    packet.prepare_write(0x1, addr, data);
    let packet = NiosPkt::<u8, u16>::new(&mut buf).unwrap();
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}

#[test]
fn packet8x32_new() {
    let addr = 0x12;
    let data = 0x12345678;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u8, u32>::new(&mut buf).unwrap();
    packet.prepare_write(0x1, addr, data);
    let packet = NiosPkt::<u8, u32>::new(&mut buf).unwrap();
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}

#[test]
fn packet8x64_new() {
    let addr = 0x12;
    let data = 0x123456789abcdef;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u8, u64>::new(&mut buf).unwrap();
    packet.prepare_write(0x1, addr, data);
    let packet = NiosPkt::<u8, u64>::new(&mut buf).unwrap();
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}

#[test]
fn packet16x64_new() {
    let addr = 0x1234;
    let data = 0x123456789abcdef;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u16, u64>::new(&mut buf).unwrap();
    packet.prepare_write(0x1, addr, data);
    let packet = NiosPkt::<u16, u64>::new(&mut buf).unwrap();
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}

#[test]
fn packet32x32_new() {
    let addr = 0x12345678;
    let data = 0x12345678;

    let mut buf = make_buf();
    let mut packet = NiosPkt::<u32, u32>::new(&mut buf).unwrap();
    packet.prepare_write(0x1, addr, data);
    let packet = NiosPkt::<u32, u32>::new(&mut buf).unwrap();
    assert_eq!(addr, packet.addr());
    assert_eq!(data, packet.data());
}
