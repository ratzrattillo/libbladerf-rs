//! Unit tests for MockTransport demonstrating NiosCore testing without hardware.

use libbladerf_rs::nios_client::NiosCore;
use libbladerf_rs::protocol::nios::{NiosPkt, NiosPktFlags, NiosPktStatus};
use libbladerf_rs::transport::Transport;
use libbladerf_rs::transport::mock::MockTransport;

/// Create a success response for a read operation with the given data.
fn make_read_response(data: u32) -> [u8; 16] {
    let mut buf = [0u8; 16];
    // Set magic for 8x32 packet
    buf[0] = 0x43; // MAGIC_8X32
    // Set success flag
    buf[2] = NiosPktStatus::Success as u8;
    // Set data at offset 5 (after magic, target, flags, reserved, addr)
    buf[5..9].copy_from_slice(&data.to_le_bytes());
    buf
}

#[test]
fn mock_transport_basic_workflow() {
    let mut mock = MockTransport::new();

    // Get the out buffer and verify it's cleared
    let out_buf = mock.out_buffer().unwrap();
    assert_eq!(out_buf.len(), 16);
    assert!(out_buf.iter().all(|&b| b == 0));
}

#[test]
fn mock_transport_with_nios_core_read() {
    let mut mock = MockTransport::new();

    // Configure a response for reading FPGA version (example: v0.16.0)
    let version_word: u32 = 16 << 16;
    mock.set_response(make_read_response(version_word));

    // Create NiosCore with mock transport
    let mut nios = NiosCore::new(mock);

    // Perform a read operation
    let result: u32 = nios.nios_read::<u8, u32>(0x10, 0).unwrap();
    assert_eq!(result, version_word);
}

#[test]
fn mock_transport_encodes_request_correctly() {
    let mut mock = MockTransport::new();
    mock.set_response(make_read_response(0x12345678));

    let mut nios = NiosCore::new(mock);

    // Perform a write operation
    nios.nios_write::<u8, u32>(0x10, 0, 0xDEADBEEF).unwrap();

    // Get the transport back and check the request was encoded
    let transport = nios.transport();
    let request = transport.last_request();

    // Verify packet structure
    let mut buf = *request;
    let pkt = NiosPkt::<u8, u32>::new(&mut buf);
    assert_eq!(pkt.target(), 0x10);
    assert_eq!(pkt.flags(), NiosPktFlags::Write);
    assert_eq!(pkt.addr(), 0);
    assert_eq!(pkt.data(), 0xDEADBEEF);
}

#[test]
fn mock_transport_multiple_operations() {
    let mut mock = MockTransport::new();

    // First operation: read
    mock.set_response(make_read_response(0x11111111));
    let mut nios = NiosCore::new(mock);

    let val1: u32 = nios.nios_read::<u8, u32>(0x10, 0).unwrap();
    assert_eq!(val1, 0x11111111);

    // Second operation: write
    nios.transport_mut().set_response(make_read_response(0));
    nios.nios_write::<u8, u32>(0x10, 1, 0x22222222).unwrap();

    // Third operation: read different address
    nios.transport_mut()
        .set_response(make_read_response(0x33333333));
    let val3: u32 = nios.nios_read::<u8, u32>(0x10, 2).unwrap();
    assert_eq!(val3, 0x33333333);
}
