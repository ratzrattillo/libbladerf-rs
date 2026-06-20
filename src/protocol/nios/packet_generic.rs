//! Generic NIOS packet builder and decoder.
//!
//! Provides `NiosPkt`, a type-parameterized wrapper over a 16-byte buffer
//! that encodes/decodes NIOS register read/write packets based on address
//! and data width. Also provides `NiosNum` for supported numeric types
//! and `NiosPacket` for raw byte-level access.

use crate::error::Result;
use crate::protocol::nios::NiosPacketError;
use std::fmt::Debug;
use std::marker::PhantomData;

/// NIOS packet read/write flag.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPktFlags {
    /// Read operation.
    Read = 0x0,
    /// Write operation.
    Write = 0x1,
}
impl From<u8> for NiosPktFlags {
    fn from(v: u8) -> Self {
        if (v & 0x01) != 0 {
            NiosPktFlags::Write
        } else {
            NiosPktFlags::Read
        }
    }
}

/// NIOS packet status flag.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NiosPktStatus {
    /// The operation completed successfully.
    Success = 0x02,
}

/// A numeric type supported as a NIOS packet address or data field.
///
/// Each implementation specifies `SIZE` (the byte width) and the
/// corresponding byte array type for little-endian conversion.
pub trait NiosNum: Sized + Copy + Debug + Default + Send {
    /// Number of bytes this type occupies in a NIOS packet.
    const SIZE: usize;
    /// Little-endian byte representation.
    type Bytes: AsRef<[u8]> + AsMut<[u8]> + Default + Copy;
    /// Converts the value to little-endian bytes.
    fn to_le_bytes(self) -> Self::Bytes;
    /// Converts little-endian bytes to the value.
    fn from_le_bytes(bytes: Self::Bytes) -> Self;
}

/// Byte-level access to a NIOS packet buffer.
///
/// Provides little-endian read/write helpers for `u16` and `u64`
/// fields at arbitrary offsets within the packet.
pub trait NiosPacket {
    /// Returns the packet buffer as an immutable slice.
    fn as_slice(&self) -> &[u8];
    /// Returns the packet buffer as a mutable slice.
    fn as_slice_mut(&mut self) -> &mut [u8];
    /// Reads a `u64` from the buffer at `offset` in little-endian order.
    fn read_u64(&self, offset: usize) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.as_slice()[offset..offset + 8]);
        u64::from_le_bytes(bytes)
    }
    /// Writes a `u64` to the buffer at `offset` in little-endian order.
    fn write_u64(&mut self, offset: usize, value: u64) {
        self.as_slice_mut()[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
    /// Reads a `u16` from the buffer at `offset` in little-endian order.
    fn read_u16(&self, offset: usize) -> u16 {
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&self.as_slice()[offset..offset + 2]);
        u16::from_le_bytes(bytes)
    }
    /// Writes a `u16` to the buffer at `offset` in little-endian order.
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

/// Type-parameterized NIOS packet builder.
///
/// Wraps a mutable 16-byte buffer and provides methods to construct
/// read or write packets. The generic parameters `A` (address type)
/// and `D` (data type) determine the packet's magic byte and field layout.
#[derive(Debug)]
pub struct NiosPkt<'a, A, D> {
    /// The underlying packet buffer (at least 16 bytes).
    buf: &'a mut [u8],
    phantom: PhantomData<(A, D)>,
}
impl<'a, A: NiosNum, D: NiosNum> NiosPkt<'a, A, D> {
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
    const NIOS_PKT_SIZE: usize = 16;
    const fn magic() -> Option<u8> {
        match (A::SIZE, D::SIZE) {
            (1, 1) => Some(Self::MAGIC_8X8),
            (1, 2) => Some(Self::MAGIC_8X16),
            (1, 4) => Some(Self::MAGIC_8X32),
            (1, 8) => Some(Self::MAGIC_8X64),
            (2, 8) => Some(Self::MAGIC_16X64),
            (4, 4) => Some(Self::MAGIC_32X32),
            _ => None,
        }
    }
    /// Creates a new `NiosPkt` from a buffer.
    ///
    /// Validates that `buf` is at least 16 bytes and that the
    /// `(A, D)` type combination has a defined magic byte.
    pub fn new(buf: &'a mut [u8]) -> Result<Self> {
        if buf.len() < Self::NIOS_PKT_SIZE {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        let _magic = Self::magic().ok_or(NiosPacketError::InvalidTypeCombination)?;
        Ok(Self {
            buf: &mut buf[..Self::NIOS_PKT_SIZE],
            phantom: PhantomData,
        })
    }
    /// Populates the packet as a read request.
    ///
    /// Sets the magic byte, target, read flag, and address field.
    pub fn prepare_read(&mut self, target: u8, addr: A) {
        self.set_magic();
        self.set_target(target);
        self.set_flags(NiosPktFlags::Read);
        self.set_addr(addr);
    }
    /// Populates the packet as a write request.
    ///
    /// Sets the magic byte, target, write flag, address, and data fields.
    pub fn prepare_write(&mut self, target: u8, addr: A, data: D) {
        self.set_magic();
        self.set_target(target);
        self.set_flags(NiosPktFlags::Write);
        self.set_addr(addr);
        self.set_data(data);
    }
    fn set_magic(&mut self) {
        self.buf[Self::IDX_MAGIC] = Self::magic().unwrap();
    }
    fn set_target(&mut self, target: u8) {
        self.buf[Self::IDX_TARGET] = target;
    }
    fn set_flags(&mut self, flags: NiosPktFlags) {
        self.buf[Self::IDX_FLAGS] = flags as u8;
    }
    fn set_addr(&mut self, addr: A) {
        self.buf[Self::IDX_ADDR..Self::IDX_ADDR + A::SIZE]
            .copy_from_slice(addr.to_le_bytes().as_ref());
    }
    fn set_data(&mut self, data: D) {
        let data_offset = Self::IDX_ADDR + A::SIZE;
        self.buf[data_offset..data_offset + D::SIZE].copy_from_slice(data.to_le_bytes().as_ref());
    }
    /// Returns the target field of the packet.
    pub fn target(&self) -> u8 {
        self.buf[Self::IDX_TARGET]
    }
    /// Returns the read/write flags of the packet.
    pub fn flags(&self) -> NiosPktFlags {
        self.buf[Self::IDX_FLAGS].into()
    }
    /// Returns the address field of the packet.
    pub fn addr(&self) -> A {
        let mut bytes: A::Bytes = Default::default();
        bytes
            .as_mut()
            .copy_from_slice(&self.buf[Self::IDX_ADDR..Self::IDX_ADDR + A::SIZE]);
        A::from_le_bytes(bytes)
    }
    /// Returns the data field of the packet.
    pub fn data(&self) -> D {
        let data_offset = Self::IDX_ADDR + A::SIZE;
        let mut bytes: D::Bytes = Default::default();
        bytes
            .as_mut()
            .copy_from_slice(&self.buf[data_offset..data_offset + D::SIZE]);
        D::from_le_bytes(bytes)
    }
    /// Returns `true` if the success status flag is set in the packet.
    pub fn is_success(&self) -> bool {
        (self.buf[Self::IDX_FLAGS] & (NiosPktStatus::Success as u8)) != 0
    }
}
impl<'a, A: NiosNum, D: NiosNum> NiosPacket for NiosPkt<'a, A, D> {
    fn as_slice(&self) -> &[u8] {
        self.buf
    }
    fn as_slice_mut(&mut self) -> &mut [u8] {
        self.buf
    }
}

/// Stateless decoder for NIOS response packets.
///
/// Extracts the data field from a raw response buffer based on the
/// generic address and data type widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct NiosPktDecoder;
impl NiosPktDecoder {
    /// Decodes the data field from a NIOS response buffer.
    ///
    /// Calculates the data offset from the address width `A::SIZE`
    /// and reads `D::SIZE` bytes of little-endian data.
    pub fn decode_data<A: NiosNum, D: NiosNum>(buf: &[u8]) -> Result<D> {
        const IDX_ADDR: usize = 4;
        let data_offset = IDX_ADDR + A::SIZE;
        if buf.len() < data_offset + D::SIZE {
            return Err(NiosPacketError::InvalidSize(buf.len()).into());
        }
        let mut bytes: D::Bytes = Default::default();
        bytes
            .as_mut()
            .copy_from_slice(&buf[data_offset..data_offset + D::SIZE]);
        Ok(D::from_le_bytes(bytes))
    }
}
