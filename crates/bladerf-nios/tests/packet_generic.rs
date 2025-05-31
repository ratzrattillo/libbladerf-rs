#[cfg(test)]
mod tests {
    /*
       Test if the fields in the generic packets are assigned correctly.
       TODO: Test boundries of fields e.g. limit of  bits is not exceeded.
    */
    use bladerf_nios::packet_base::GenericNiosPkt;
    use bladerf_nios::packet_generic::{
        NiosPkt8x8, NiosPkt8x16, NiosPkt8x32, NiosPkt8x64, NiosPkt16x64, NiosPkt32x32,
    };

    #[test]
    fn packet_reuse() {
        type PktType = NiosPkt8x8;

        let packet = PktType::new(1, 2, 3, 4);

        let packet_vec: Vec<u8> = packet.into();
        let ptr1 = packet_vec.as_ptr();

        let reused_packet = PktType::from(packet_vec);
        let ptr2 = reused_packet.buf_ptr();

        assert_eq!(ptr1, ptr2);
    }
    #[test]
    fn packet_from_vec() {
        type PktType = NiosPkt8x8;

        let packet_vec = vec![0u8; 16];
        let mut packet = PktType::from(packet_vec);
        packet.set_magic(PktType::MAGIC);

        // assert_eq!(Ok(()), packet.validate());
        assert_eq!(PktType::MAGIC, packet.magic());
        assert_eq!(0x0, packet.target_id());
        assert_eq!(0x0, packet.flags());
        assert_eq!(0x0, packet.addr());
        assert_eq!(0x0, packet.data());
    }

    #[test]
    fn packet8x8_into() {
        type PktType = NiosPkt8x8;

        let target_id = 0x1;
        let flags = 0x2;
        let addr = 0x12;
        let data = 0x12;

        let packet = PktType::new(target_id, flags, addr, data);
        let packet_vec: Vec<u8> = packet.into();

        assert_eq!(packet_vec.magic(), PktType::MAGIC);
        assert_eq!(packet_vec[PktType::IDX_TARGET_ID], target_id);
        assert_eq!(packet_vec[PktType::IDX_FLAGS], flags);
        assert_eq!(packet_vec[PktType::IDX_RESERVED], 0x00);
        assert_eq!(packet_vec[PktType::IDX_ADDR], addr);
        assert_eq!(packet_vec[PktType::IDX_DATA], data);
        let padding = &packet_vec[PktType::IDX_PADDING..];
        assert_eq!(padding, vec![0u8; padding.len()].as_slice());
    }

    #[test]
    fn packet8x8_new() {
        type PktType = NiosPkt8x8;

        let target_id = 0x1;
        let flags = 0x2;
        let addr = 0x12;
        let data = 0x12;

        let packet = PktType::new(target_id, flags, addr, data);
        assert_eq!(PktType::MAGIC, packet.magic());
        assert_eq!(target_id, packet.target_id());
        assert_eq!(flags, packet.flags());
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x16_new() {
        type PktType = NiosPkt8x16;

        let addr = 0x12;
        let data = 0x1234;

        let packet = PktType::new(0x1, 0x2, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x32_new() {
        type PktType = NiosPkt8x32;

        let addr = 0x12;
        let data = 0x12345678;

        let packet = PktType::new(0x1, 0x2, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet8x64_new() {
        type PktType = NiosPkt8x64;

        let addr = 0x12;
        let data = 0x123456789abcdef;

        let packet = PktType::new(0x1, 0x2, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet16x64_new() {
        type PktType = NiosPkt16x64;

        let addr = 0x1234;
        let data = 0x123456789abcdef;

        let packet = PktType::new(0x1, 0x2, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }

    #[test]
    fn packet32x32_new() {
        type PktType = NiosPkt32x32;

        let addr = 0x12345678;
        let data = 0x12345678;

        let packet = PktType::new(0x1, 0x2, addr, data);
        assert_eq!(addr, packet.addr());
        assert_eq!(data, packet.data());
    }
}
