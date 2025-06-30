pub trait GenericNiosPkt {
    /*
        This trait contains methods that are used by more than one packet type, to avoid
        code duplication.
    */
    const IDX_MAGIC: usize = 0;
    const IDX_TIME: usize = 1;

    fn magic(&self) -> u8;
    fn set_magic(&mut self, magic: u8) -> &mut Self;

    fn duration_or_timestamp(&self) -> u64;
    fn set_duration_or_timestamp(&mut self, timestamp: u64) -> &mut Self;
}

impl GenericNiosPkt for Vec<u8> {
    fn magic(&self) -> u8 {
        self[Self::IDX_MAGIC]
    }

    fn set_magic(&mut self, magic: u8) -> &mut Self {
        self[Self::IDX_MAGIC] = magic;
        self
    }

    fn duration_or_timestamp(&self) -> u64 {
        let pkt_mem = &self[Self::IDX_TIME..Self::IDX_TIME + 8];
        u64::from_le_bytes(pkt_mem.try_into().unwrap())
    }

    fn set_duration_or_timestamp(&mut self, timestamp: u64) -> &mut Self {
        self[Self::IDX_TIME..Self::IDX_TIME + 8]
            .copy_from_slice(timestamp.to_le_bytes().as_slice());
        self
    }
}
