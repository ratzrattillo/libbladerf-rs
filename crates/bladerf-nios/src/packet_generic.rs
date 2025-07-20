use crate::NiosPktMagic;
use crate::packet_base::GenericNiosPkt;
use bladerf_globals::{Error, Result};
use std::fmt::{Debug, Display, Formatter, LowerHex};
// pub const NIOS_PKT_8X8_MAGIC: u8 = 0x41; // 'A'
// pub const NIOS_PKT_8X16_MAGIC: u8 = 0x42; // 'B'
// pub const NIOS_PKT_8X32_MAGIC: u8 = 0x43; // 'C'
// pub const NIOS_PKT_8X64_MAGIC: u8 = 0x44; // 'D'
// pub const NIOS_PKT_16X64_MAGIC: u8 = 0x45; // 'E'
// pub const NIOS_PKT_32X32_MAGIC: u8 = 0x4B; // 'K'

pub type NiosReq8x8 = NiosPktRequest<u8, u8>;
pub type NiosReq8x16 = NiosPktRequest<u8, u16>;
pub type NiosReq8x32 = NiosPktRequest<u8, u32>;
pub type NiosReq8x64 = NiosPktRequest<u8, u64>;
pub type NiosReq16x64 = NiosPktRequest<u16, u64>;
pub type NiosReq32x32 = NiosPktRequest<u32, u32>;

pub type NiosResp8x8 = NiosPktResponse<u8, u8>;
pub type NiosResp8x16 = NiosPktResponse<u8, u16>;
pub type NiosResp8x32 = NiosPktResponse<u8, u32>;
pub type NiosResp8x64 = NiosPktResponse<u8, u64>;
pub type NiosResp16x64 = NiosPktResponse<u16, u64>;
pub type NiosResp32x32 = NiosPktResponse<u32, u32>;

// https://stackoverflow.com/questions/78395612/how-to-enforce-generic-parameter-to-be-of-type-u8-u16-u32-or-u64-in-rust
// https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/
pub trait NumToByte {
    // https://github.com/rust-lang/rust/issues/60551
    // type DataType;
    // fn to_le_bytes(&self) -> [u8; size_of::<Self::DataType>()];
    fn to_le_bytes_vec(&self) -> Vec<u8>;
    fn from_le_bytes(bytes: &[u8]) -> Self;
}

impl NumToByte for u8 {
    // type DataType = u8;

    // fn to_le_bytes(&self) -> [u8; size_of::<Self::DataType>()] {
    //     Self::DataType::to_le_bytes(*self)
    // }
    fn to_le_bytes_vec(&self) -> Vec<u8> {
        Self::to_le_bytes(*self).to_vec()
    }

    fn from_le_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes(bytes.try_into().expect("slice with incorrect length"))
    }
}
impl NumToByte for u16 {
    fn to_le_bytes_vec(&self) -> Vec<u8> {
        Self::to_le_bytes(*self).to_vec()
    }

    fn from_le_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes(bytes.try_into().expect("slice with incorrect length"))
    }
}
impl NumToByte for u32 {
    fn to_le_bytes_vec(&self) -> Vec<u8> {
        Self::to_le_bytes(*self).to_vec()
    }

    fn from_le_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes(bytes.try_into().expect("slice with incorrect length"))
    }
}
impl NumToByte for u64 {
    fn to_le_bytes_vec(&self) -> Vec<u8> {
        Self::to_le_bytes(*self).to_vec()
    }

    fn from_le_bytes(bytes: &[u8]) -> Self {
        Self::from_le_bytes(bytes.try_into().expect("slice with incorrect length"))
    }
}

pub struct NiosPkt<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pub buf: Vec<u8>,
    phantom: std::marker::PhantomData<(A, D)>,
}

impl<A, D> NiosPkt<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pub const IDX_TARGET_ID: usize = 1;
    pub const IDX_FLAGS: usize = 2;
    pub const IDX_ADDR: usize = 4;
    pub const IDX_DATA: usize = Self::IDX_ADDR + size_of::<A>();
    // pub const IDX_PADDING: usize = Self::IDX_DATA + size_of::<D>();

    pub const FLAG_READ: u8 = 0;
    pub const FLAG_WRITE: u8 = 1;
    pub const FLAG_SUCCESS: u8 = 2;

    pub const MAGIC: u8 = match (size_of::<A>(), size_of::<D>()) {
        (1, 1) => NiosPktMagic::_8X8 as u8,
        (1, 2) => NiosPktMagic::_8X16 as u8,
        (1, 4) => NiosPktMagic::_8X32 as u8,
        (1, 8) => NiosPktMagic::_8X64 as u8,
        (2, 8) => NiosPktMagic::_16X64 as u8,
        (4, 4) => NiosPktMagic::_32X32 as u8,
        _ => {
            // log::error!("Wrong type sizes for NIOS packet");
            panic!("Wrong type sizes for NIOS packet")
        }
    };

    pub fn buf_ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    pub fn new(target_id: u8, flags: u8, addr: A, data: D) -> Self {
        let mut pkt: NiosPkt<A, D> = vec![0u8; 16].into();
        pkt.set(target_id, flags, addr, data);
        pkt
    }

    pub fn set(&mut self, target_id: u8, flags: u8, addr: A, data: D) -> &mut Self {
        self.set_magic(Self::MAGIC)
            .set_target_id(target_id)
            .set_flags(flags)
            .set_addr(addr)
            .set_data(data)
    }
    pub fn magic(&self) -> u8 {
        self.buf.magic()
    }
    pub fn target_id(&self) -> u8 {
        self.buf[Self::IDX_TARGET_ID]
    }
    pub fn flags(&self) -> u8 {
        self.buf[Self::IDX_FLAGS]
    }
    pub fn addr(&self) -> A {
        A::from_le_bytes(&self.buf[Self::IDX_ADDR..(Self::IDX_ADDR + size_of::<A>())])
    }
    pub fn data(&self) -> D {
        D::from_le_bytes(&self.buf[Self::IDX_DATA..(Self::IDX_DATA + size_of::<D>())])
    }
    pub fn is_success(&self) -> Result<()> {
        if self.buf[Self::IDX_FLAGS] & Self::FLAG_SUCCESS == 0 {
            log::error!("response packet reported failure.");
            return Err(Error::Invalid);
        }
        Ok(())
    }
    pub fn write(&self) -> bool {
        self.buf[Self::IDX_FLAGS] & Self::FLAG_WRITE != 0
    }

    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        // self.buf[Self::IDX_MAGIC] = magic;
        self.buf.set_magic(magic);
        self
    }
    pub fn set_target_id(&mut self, target_id: u8) -> &mut Self {
        self.buf[Self::IDX_TARGET_ID] = target_id;
        self
    }
    pub fn set_flag(&mut self, flag: u8) -> &mut Self {
        self.buf[Self::IDX_FLAGS] |= flag;
        self
    }
    pub fn set_flags(&mut self, flags: u8) -> &mut Self {
        self.buf[Self::IDX_FLAGS] = flags;
        self
    }
    pub fn set_addr(&mut self, addr: A) -> &mut Self {
        self.buf[Self::IDX_ADDR..(Self::IDX_ADDR + size_of::<A>())]
            .copy_from_slice(addr.to_le_bytes_vec().as_slice());
        self
    }
    pub fn set_data(&mut self, data: D) -> &mut Self {
        self.buf[Self::IDX_DATA..(Self::IDX_DATA + size_of::<D>())]
            .copy_from_slice(data.to_le_bytes_vec().as_slice());
        self
    }

    // pub fn validate(&self) -> Result<(), ValidationError> {
    //     if self.magic() != Self::MAGIC {
    //         return Err(ValidationError::InvalidMagic(self.magic()));
    //     }
    //     if self.reserved() != 0x00 {
    //         return Err(ValidationError::InvalidReserved(self.reserved()));
    //     }
    //     if self.padding().iter().any(|x|///x != 0) {
    //         return Err(ValidationError::InvalidPadding(self.padding().to_vec()));
    //     }
    //     if self.buf.len() != 16 {
    //         return Err(ValidationError::InvalidLength(self.buf.len()));
    //     }
    //     Ok(())
    // }
}

impl<A, D> From<Vec<u8>> for NiosPkt<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: Vec<u8>) -> Self {
        Self {
            buf: value,
            phantom: Default::default(),
        }
    }
}

impl<A, D> From<NiosPkt<A, D>> for Vec<u8>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: NiosPkt<A, D>) -> Self {
        value.buf
    }
}

impl<A, D> Debug for NiosPkt<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // let magic = match self.magic() {
        //     NIOS_PKT_8X8_MAGIC => "Nios_8x8",
        //     NIOS_PKT_8X16_MAGIC => "Nios_8x16",
        //     NIOS_PKT_8X32_MAGIC => "Nios_8x32",
        //     NIOS_PKT_8X64_MAGIC => "Nios_8x64",
        //     NIOS_PKT_16X64_MAGIC => "Nios_16x64",
        //     NIOS_PKT_32X32_MAGIC => "Nios_32x32",
        //     _ => "UNKNOWN",
        // };
        let flags = match self.flags() {
            0x0 => "READ FAILURE",
            0x1 => "WRITE FAILURE",
            0x2 => "READ SUCCESS",
            0x3 => "WRITE SUCCESS",
            _ => "UNKNOWN",
        };
        // let flags = match self.flags() {
        //     0x0 => NiosPktFlags::ReadFailure,
        //     0x1 => NiosPktFlags::WriteFailure,
        //     0x2 => NiosPktFlags::ReadSuccess,
        //     0x3 => NiosPktFlags::WriteSuccess,
        // };
        f.debug_struct("NiosPkt")
            .field("magic", &format_args!("{:#x}", self.magic()))
            .field("target", &format_args!("{:#x}", self.target_id()))
            .field("flags", &String::from(flags))
            .field("addr", &format_args!("{:#x}", self.addr()))
            .field("data", &format_args!("{:#x}", self.data()))
            .finish()
    }
}

impl<A, D> Display for NiosPkt<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for elem in self.buf.iter() {
            f.write_fmt(format_args!("{elem:02x} "))?;
        }
        f.write_str("]")?;
        Ok(())
    }
}

/// This file defines the Host <-> FPGA (NIOS II) packet formats for accesses
/// to devices/blocks with X-bit addresses and Y-bit data, where X and Y are
/// a multiple of 8.
///
///
///                              Request
///                      ----------------------
///
/// +================+=========================================================+
/// |  Byte offset   |                       Description                       |
/// +================+=========================================================+
/// |        0       | Magic Value                                             |
/// +----------------+---------------------------------------------------------+
/// |        1       | Target ID (Note 1)                                      |
/// +----------------+---------------------------------------------------------+
/// |        2       | Flags (Note 2)                                          |
/// +----------------+---------------------------------------------------------+
/// |        3       | Reserved. Set to 0x00.                                  |
/// +----------------+---------------------------------------------------------+
/// |        4       | X-bit address                                           |
/// +----------------+---------------------------------------------------------+
/// |        5       | Y-bit data                                              |
/// +----------------+---------------------------------------------------------+
/// |      15:6      | Reserved. Set to 0.                                     |
/// +----------------+---------------------------------------------------------+
///
///
///                              Response
///                      ----------------------
///
/// The response packet contains the same information as the request.
/// A status flag will be set if the operation is completed successfully.
///
/// In the case of a read request, the data field will contain the read data if
/// the read succeeded.
///
/// (Note 1)
///  The "Target ID" refers to the peripheral, device, or block to access.
///  See the NIOS_PKT_XxY_TARGET_* values.
///
/// (Note 2)
///  The flags are defined as follows:
///
///    +================+========================+
///    |      Bit(s)    |         Value          |
///    +================+========================+
///    |       7:2      | Reserved. Set to 0.    |
///    +----------------+------------------------+
///    |                | Status. Only used in   |
///    |                | response packet.       |
///    |                | Ignored in request.    |
///    |        1       |                        |
///    |                |   1 = Success          |
///    |                |   0 = Failure          |
///    +----------------+------------------------+
///    |        0       |   0 = Read operation   |
///    |                |   1 = Write operation  |
///    +----------------+------------------------+
///
pub struct NiosPktRequest<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pkt: NiosPkt<A, D>,
}
impl<A, D> NiosPktRequest<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pub const FLAG_READ: u8 = 0;
    pub const FLAG_WRITE: u8 = 1;
    pub const FLAG_SUCCESS: u8 = 2;

    pub fn new(target_id: u8, flags: u8, addr: A, data: D) -> Self {
        Self {
            pkt: NiosPkt::new(target_id, flags, addr, data),
        }
    }
    pub fn set(&mut self, target_id: u8, flags: u8, addr: A, data: D) -> &mut Self {
        self.pkt.set(target_id, flags, addr, data);
        self
    }
    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        self.pkt.set_magic(magic);
        self
    }
    pub fn set_target_id(&mut self, target_id: u8) -> &mut Self {
        self.pkt.set_target_id(target_id);
        self
    }
    pub fn set_flag(&mut self, flag: u8) -> &mut Self {
        self.pkt.set_flag(flag);
        self
    }
    pub fn set_flags(&mut self, flags: u8) -> &mut Self {
        self.pkt.set_flags(flags);
        self
    }

    pub fn set_addr(&mut self, addr: A) -> &mut Self {
        self.pkt.set_addr(addr);
        self
    }
    pub fn set_data(&mut self, data: D) -> &mut Self {
        self.pkt.set_data(data);
        self
    }

    pub fn buf_ptr(&self) -> *const u8 {
        self.pkt.buf_ptr()
    }

    pub fn magic(&self) -> u8 {
        self.pkt.magic()
    }
    pub fn target_id(&self) -> u8 {
        self.pkt.target_id()
    }
    pub fn flags(&self) -> u8 {
        self.pkt.flags()
    }
    pub fn addr(&self) -> A {
        self.pkt.addr()
    }
    pub fn data(&self) -> D {
        self.pkt.data()
    }
    pub fn is_success(&self) -> Result<()> {
        self.pkt.is_success()
    }
    pub fn is_write(&self) -> bool {
        self.pkt.write()
    }
}

impl<A, D> From<Vec<u8>> for NiosPktRequest<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: Vec<u8>) -> Self {
        Self {
            pkt: NiosPkt::from(value),
        }
    }
}

impl<A, D> From<NiosPktRequest<A, D>> for Vec<u8>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: NiosPktRequest<A, D>) -> Self {
        value.pkt.buf
    }
}

impl<A, D> Debug for NiosPktRequest<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.pkt, f)
    }
}

impl<A, D> Display for NiosPktRequest<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.pkt, f)
    }
}

pub struct NiosPktResponse<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pkt: NiosPkt<A, D>,
}

impl<A, D> NiosPktResponse<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    pub fn magic(&self) -> u8 {
        self.pkt.magic()
    }
    pub fn target_id(&self) -> u8 {
        self.pkt.target_id()
    }
    pub fn flags(&self) -> u8 {
        self.pkt.flags()
    }
    pub fn addr(&self) -> A {
        self.pkt.addr()
    }
    pub fn data(&self) -> D {
        self.pkt.data()
    }
    pub fn is_success(&self) -> Result<()> {
        self.pkt.is_success()
    }
    pub fn is_write(&self) -> bool {
        self.pkt.write()
    }
}

impl<A, D> From<Vec<u8>> for NiosPktResponse<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: Vec<u8>) -> Self {
        Self {
            pkt: NiosPkt::from(value),
        }
    }
}

impl<A, D> From<NiosPktResponse<A, D>> for Vec<u8>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn from(value: NiosPktResponse<A, D>) -> Self {
        value.pkt.buf
    }
}

impl<A, D> Debug for NiosPktResponse<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.pkt, f)
    }
}

impl<A, D> Display for NiosPktResponse<A, D>
where
    A: NumToByte + Debug + Display + LowerHex,
    D: NumToByte + Debug + Display + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.pkt, f)
    }
}
