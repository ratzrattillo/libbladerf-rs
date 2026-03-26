use crate::{Error, Result};
use std::fmt::Debug;
use std::marker::PhantomData;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPktFlags {
    Read = 0x0,
    Write = 0x1,
}

impl From<u8> for NiosPktFlags {
    fn from(v: u8) -> Self {
        if v & 0x01 != 0 {
            NiosPktFlags::Write
        } else {
            NiosPktFlags::Read
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPktStatus {
    Success = 0x02,
}

pub trait NiosNum: Sized + Copy + Debug + Default + Send {
    const SIZE: usize;
    type Bytes: AsRef<[u8]> + AsMut<[u8]> + Default + Copy;
    fn to_le_bytes(self) -> Self::Bytes;
    fn from_le_bytes(bytes: Self::Bytes) -> Self;
}

pub trait NiosPacket: Sized {
    fn as_slice(&self) -> &[u8];

    fn as_slice_mut(&mut self) -> &mut [u8];

    fn into_inner(self) -> Vec<u8>;

    fn into_packet(self) -> Vec<u8>;

    fn read_u64(&self, offset: usize) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.as_slice()[offset..offset + 8]);
        u64::from_le_bytes(bytes)
    }

    fn write_u64(&mut self, offset: usize, value: u64) {
        self.as_slice_mut()[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn read_u16(&self, offset: usize) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.as_slice()[offset..offset + 2]);
        u16::from_le_bytes(bytes)
    }

    fn write_u16(&mut self, offset: usize, value: u16) {
        self.as_slice_mut()[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
}

macro_rules! impl_nios_num {
    ($t:ty, $n:literal) => {
        impl NiosNum for $t {
            const SIZE: usize = $n;
            type Bytes = [u8; $n];
            fn to_le_bytes(self) -> [u8; $n] {
                <$t>::to_le_bytes(self)
            }
            fn from_le_bytes(bytes: [u8; $n]) -> Self {
                <$t>::from_le_bytes(bytes)
            }
        }
    };
}

impl_nios_num!(u8, 1);
impl_nios_num!(u16, 2);
impl_nios_num!(u32, 4);
impl_nios_num!(u64, 8);

#[derive(Debug)]
pub struct NiosPkt<A, D> {
    buf: Vec<u8>,
    phantom: PhantomData<(A, D)>,
}

impl<A: NiosNum, D: NiosNum> NiosPkt<A, D> {
    const IDX_MAGIC: usize = 0;
    const IDX_TARGET: usize = 1;
    const IDX_FLAGS: usize = 2;
    const IDX_ADDR: usize = 4;

    const MAGIC_8X8: u8 = 0x41;
    const MAGIC_8X16: u8 = 0x42;
    const MAGIC_8X32: u8 = 0x43;
    const MAGIC_8X64: u8 = 0x44;
    const MAGIC_16X64: u8 = 0x45;
    const MAGIC_32X32: u8 = 0x4B;

    const fn magic() -> u8 {
        match (A::SIZE, D::SIZE) {
            (1, 1) => Self::MAGIC_8X8,
            (1, 2) => Self::MAGIC_8X16,
            (1, 4) => Self::MAGIC_8X32,
            (1, 8) => Self::MAGIC_8X64,
            (2, 8) => Self::MAGIC_16X64,
            (4, 4) => Self::MAGIC_32X32,
            _ => panic!("unsupported address/data size combination"),
        }
    }

    pub fn prepare_read(self, target: u8, addr: A) -> Self {
        self.set_magic()
            .set_target(target)
            .set_flags(NiosPktFlags::Read)
            .set_addr(addr)
    }

    pub fn prepare_write(self, target: u8, addr: A, data: D) -> Self {
        self.set_magic()
            .set_target(target)
            .set_flags(NiosPktFlags::Write)
            .set_addr(addr)
            .set_data(data)
    }

    fn set_magic(mut self) -> Self {
        self.buf[Self::IDX_MAGIC] = Self::magic();
        self
    }

    fn set_target(mut self, target: u8) -> Self {
        self.buf[Self::IDX_TARGET] = target;
        self
    }

    fn set_flags(mut self, flags: NiosPktFlags) -> Self {
        self.buf[Self::IDX_FLAGS] = flags as u8;
        self
    }

    fn set_addr(mut self, addr: A) -> Self {
        self.buf[Self::IDX_ADDR..Self::IDX_ADDR + A::SIZE]
            .copy_from_slice(addr.to_le_bytes().as_ref());
        self
    }

    fn set_data(mut self, data: D) -> Self {
        let data_offset = Self::IDX_ADDR + A::SIZE;
        self.buf[data_offset..data_offset + D::SIZE].copy_from_slice(data.to_le_bytes().as_ref());
        self
    }

    pub fn target(&self) -> u8 {
        self.buf[Self::IDX_TARGET]
    }

    pub fn flags(&self) -> NiosPktFlags {
        self.buf[Self::IDX_FLAGS].into()
    }

    pub fn addr(&self) -> A {
        let mut bytes: A::Bytes = Default::default();
        bytes
            .as_mut()
            .copy_from_slice(&self.buf[Self::IDX_ADDR..Self::IDX_ADDR + A::SIZE]);
        A::from_le_bytes(bytes)
    }

    pub fn data(&self) -> D {
        let data_offset = Self::IDX_ADDR + A::SIZE;
        let mut bytes: D::Bytes = Default::default();
        bytes
            .as_mut()
            .copy_from_slice(&self.buf[data_offset..data_offset + D::SIZE]);
        D::from_le_bytes(bytes)
    }

    pub fn is_write(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & (NiosPktFlags::Write as u8) != 0
    }

    pub fn is_read(&self) -> bool {
        !self.is_write()
    }

    pub fn is_success(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & (NiosPktStatus::Success as u8) != 0
    }
}

impl<A: NiosNum, D: NiosNum> NiosPacket for NiosPkt<A, D> {
    fn as_slice(&self) -> &[u8] {
        &self.buf
    }

    fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    fn into_packet(self) -> Vec<u8> {
        let mut buf = self.buf;
        buf.truncate(16);
        buf
    }
}

impl<A: NiosNum, D: NiosNum> TryFrom<Vec<u8>> for NiosPkt<A, D> {
    type Error = Error;

    fn try_from(buf: Vec<u8>) -> Result<Self> {
        Ok(Self {
            buf,
            phantom: PhantomData,
        })
    }
}
