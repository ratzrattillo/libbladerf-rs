#[cfg(test)]
mod tests {
    use libbladerf_rs::protocol::nios::{NiosPacket, NiosPkt, NiosPktFlags};

    fn make_buf() -> Vec<u8> {
        vec![0u8; 16]
    }

    #[test]
    fn packet_from_into_array() {
        type PktType = NiosPkt<u8, u8>;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(1, 3, 4);
        let buf = packet.into_inner();
        assert_eq!(buf[0], 0x41);

        let from_vec = PktType::try_from(buf).unwrap();
        assert_eq!(from_vec.target(), 1);
    }

    #[test]
    fn packet_from_vec() {
        type PktType = NiosPkt<u8, u8>;

        let packet_array = [0u8; 16];
        let packet = PktType::try_from(packet_array.to_vec()).unwrap();

        assert_eq!(0x0, packet.target());
        assert_eq!(NiosPktFlags::Read, packet.flags());
        assert_eq!(0x0, packet.addr());
        assert_eq!(0x0, packet.data());
    }

    #[test]
    fn packet8x8_new() {
        type PktType = NiosPkt<u8, u8>;

        let target_id = 0x1;
        let addr = 0x12;
        let data = 0x12;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(target_id, addr, data);
        assert_eq!(target_id, packet.target());
        assert_eq!(NiosPktFlags::Write, packet.flags());
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x16_new() {
        type PktType = NiosPkt<u8, u16>;

        let addr = 0x12;
        let data = 0x1234;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(0x1, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x32_new() {
        type PktType = NiosPkt<u8, u32>;

        let addr = 0x12;
        let data = 0x12345678;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(0x1, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x64_new() {
        type PktType = NiosPkt<u8, u64>;

        let addr = 0x12;
        let data = 0x123456789abcdef;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(0x1, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet16x64_new() {
        type PktType = NiosPkt<u16, u64>;

        let addr = 0x1234;
        let data = 0x123456789abcdef;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(0x1, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet32x32_new() {
        type PktType = NiosPkt<u32, u32>;

        let addr = 0x12345678;
        let data = 0x12345678;

        let packet = PktType::try_from(make_buf())
            .unwrap()
            .prepare_write(0x1, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }
}
