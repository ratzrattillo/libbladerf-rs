// #![allow(unsafe_code)]
// /* This is the original packet format used to issue requests from the
//  * host to the FPGA via the FX3 UART.
//  *
//  * This format remains supported for backwards compatibility, but should no
//  * longer be added to.
//  *
//  * If you're looking to customize the FPGA, consider using
//  * one of the "pkt_AxB" packet formats and handlers, or implementing a new
//  * format and handler.
//  *
//  *                              Request
//  *                      ----------------------
//  *
//  * +================+=========================================================+
//  * |  Byte offset   |                       Description                       |
//  * +================+=========================================================+
//  * |        0       | Magic Value                                             |
//  * +----------------+---------------------------------------------------------+
//  * |        1       | Configuration byte (Note 1)                             |
//  * +----------------+---------------------------------------------------------+
//  * |      2 - 15    | Pairs of 8-bit addr, 8-bit data                         |
//  * +----------------+---------------------------------------------------------+
//  *
//  *
//  *
//  * Note 1: Configuration byte:
//  *
//  * +================+============================+
//  * |      Bit(s)    |         Value              |
//  * +================+============================+
//  * |        7       |   1 = Read operation       |
//  * +----------------+----------------------------+
//  * |        6       |   1 = Write operation      |
//  * +----------------+----------------------------+
//  * |       5:4      | Device:                    |
//  * |                |   00 - Config PIO (Note 2) |
//  * |                |   01 - LMS register        |
//  * |                |   10 - VCTCXO Trim DAC     |
//  * |                |   11 - SI5338 register     |
//  * +----------------+----------------------------+
//  * |        3       | Unused                     |
//  * +----------------+----------------------------+
//  * |       2:0      | Addr/Data pair count       |
//  * |                | (Note 2)                   |
//  * +----------------+----------------------------+
//  *
//  * Note 2: Config PIO addresses
//  *
//  * The NIOS II core and modules in the FPGA's programmable fabric are connected
//  * via parallel IO (PIO). See the NIOS_PKT_LEGACY_PIO_ADDR_* definitions
//  * in this file contain a virtual "register map" for these modules.
//  *
//  * Note 3: "Count" field
//  *
//  * The original intent of this field was to allow multiple register
//  * accesses to be requested at once.
//  *
//  * However, this feature was not leveraged by the host code for the LMS and
//  * SI5338 accesses, so revised legacy packet handler only processes the
//  * first addr/data pair.
//  *
//  * Readback of the time tamer values is the only case where this field
//  * is set to a count greater than 1.
//  *
//  * Although config PIO values are larger than one byte, the host code
//  * accessed these byte by byte through multiple requests.  For example,
//  * 4 accesses would be required to fully read/write the configuration PIO.
//  *
//  * The above inefficiency is the motivation behind adding packet handlers
//  * that can read/write 32 or 64 bits in a single request (e.g., pkt_8x32,
//  * pkt_8x64).
//  *
//  *
//  *
//  *                              Response
//  *                      ----------------------
//  *
//  * The response for the legacy packet is essentially just the device
//  * echoing the request.
//  *
//  * On a read request, the number of requested items will be populated
//  * in bytes 2:15.
//  *
//  * The remaining bytes, or all of bytes 2:15 on a write request, should
//  * be regarded as "undefined" values and not used.
//  *
//  * +================+=========================================================+
//  * |  Byte offset   |                       Description                       |
//  * +================+=========================================================+
//  * |        0       | Magic Value                                             |
//  * +----------------+---------------------------------------------------------+
//  * |        1       | Configuration byte                                      |
//  * +----------------+---------------------------------------------------------+
//  * |      2 - 15    | Pairs of 8-bit addr, 8-bit data                         |
//  * +----------------+---------------------------------------------------------+
//  *
//  */
// use futures_lite::AsyncReadExt;
// use std::fmt::Debug;
// use std::mem::ManuallyDrop;
// use std::ops::Add;
//
// pub const NIOS_PKT_LEGACY_MAGIC: u8 = 'N' as u8;
//
// pub const NIOS_PKT_LEGACY_DEV_GPIO_ADDR: u8 = 0;
// pub const NIOS_PKT_LEGACY_DEV_RX_GAIN_ADDR: u8 = 4;
// pub const NIOS_PKT_LEGACY_DEV_RX_PHASE_ADDR: u8 = 6;
// pub const NIOS_PKT_LEGACY_DEV_TX_GAIN_ADDR: u8 = 8;
// pub const NIOS_PKT_LEGACY_DEV_TX_PHASE_ADDR: u8 = 10;
// pub const NIOS_PKT_LEGACY_DEV_FPGA_VERSION_ID: u8 = 12;
//
// pub const NIOS_PKT_LEGACY_MODE_CNT_MASK: u8 = 0x7;
// pub const NIOS_PKT_LEGACY_MODE_CNT_SHIFT: u8 = 0;
// pub const NIOS_PKT_LEGACY_MODE_DEV_MASK: u8 = 0x30;
// pub const NIOS_PKT_LEGACY_MODE_DEV_SHIFT: u8 = 4;
//
// pub const NIOS_PKT_LEGACY_DEV_CONFIG: u8 = (0 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT);
// pub const NIOS_PKT_LEGACY_DEV_LMS: u8 = (1 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT);
// pub const NIOS_PKT_LEGACY_DEV_VCTCXO: u8 = (2 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT);
// pub const NIOS_PKT_LEGACY_DEV_SI5338: u8 = (3 << NIOS_PKT_LEGACY_MODE_DEV_SHIFT);
//
// pub const NIOS_PKT_LEGACY_MODE_DIR_MASK: u8 = 0xC0;
// pub const NIOS_PKT_LEGACY_MODE_DIR_SHIFT: u8 = 6;
// pub const NIOS_PKT_LEGACY_MODE_DIR_READ: u8 = (2 << NIOS_PKT_LEGACY_MODE_DIR_SHIFT);
// pub const NIOS_PKT_LEGACY_MODE_DIR_WRITE: u8 = (1 << NIOS_PKT_LEGACY_MODE_DIR_SHIFT);
//
// /* PIO address space */
//
// /*
//  * 32-bit Device control register.
//  *
//  * This is register accessed via the libbladeRF functions,
//  * bladerf_config_gpio_write() and bladerf_config_gpio_read().
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_CONTROL: u8 = 0;
// pub const NIOS_PKT_LEGACY_PIO_LEN_CONTROL: u8 = 4;
//
// /*
//  * IQ Correction: 16-bit RX Gain value
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_IQ_RX_GAIN: u8 = 4;
// pub const NIOS_PKT_LEGACY_PIO_LEN_IQ_RX_GAIN: u8 = 2;
//
// /*
//  * IQ Correction: 16-bit RX Phase value
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_IQ_RX_PHASE: u8 = 6;
// pub const NIOS_PKT_LEGACY_PIO_LEN_IQ_RX_PHASE: u8 = 2;
//
// /*
//  * IQ Correction: 16-bit TX Gain value
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_IQ_TX_GAIN: u8 = 8;
// pub const NIOS_PKT_LEGACY_PIO_LEN_IQ_TX_GAIN: u8 = 2;
//
// /*
//  * IQ Correction: 16-bit TX Phase value
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_IQ_TX_PHASE: u8 = 10;
// pub const NIOS_PKT_LEGACY_PIO_LEN_IQ_TX_PHASE: u8 = 2;
//
// /*
//  * 32-bit FPGA Version (read-only)
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_FPGA_VERSION: u8 = 12;
// pub const NIOS_PKT_LEGACY_PIO_LEN_FPGA_VERSION: u8 = 4;
//
// /*
//  * 64-bit RX timestamp
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_RX_TIMESTAMP: u8 = 16;
// pub const NIOS_PKT_LEGACY_PIO_LEN_RX_TIMESTAMP: u8 = 8;
//
// /*
//  * 64-bit TX timestamp
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_TX_TIMESTAMP: u8 = 24;
// pub const NIOS_PKT_LEGACY_PIO_LEN_TX_TIMESTAMP: u8 = 8;
//
// /*
//  * VCTCXO Trim DAC value
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_VCTCXO: u8 = 34;
// pub const NIOS_PKT_LEGACY_PIO_LEN_VCTCXO: u8 = 2;
//
// /*
//  * XB-200 ADF4351 Synthesizer
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_XB200_SYNTH: u8 = 36;
// pub const NIOS_PKT_LEGACY_PIO_LEN_XB200_SYNTH: u8 = 4;
//
// /*
//  * Expansion IO
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_EXP: u8 = 40;
// pub const NIOS_PKT_LEGACY_PIO_LEN_EXP: u8 = 4;
//
// /*
//  * Expansion IO Direction
//  */
// pub const NIOS_PKT_LEGACY_PIO_ADDR_EXP_DIR: u8 = 44;
// pub const NIOS_PKT_LEGACY_PIO_LEN_EXP_DIR: u8 = 4;
//
// // struct uart_cmd {
// //: u8 = : u8 =  unsigned char addr;
// //: u8 = : u8 =  unsigned char data;
// // };
//
// // https://stackoverflow.com/questions/78395612/how-to-enforce-generic-parameter-to-be-of-type-u8-u16-u32-or-u64-in-rust
// // https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/
// trait Marker {}
//
// impl Marker for u8 {}
// impl Marker for u16 {}
// impl Marker for u32 {}
// impl Marker for u64 {}
//
// pub struct NiosPktLegacy<A, D>
// where
//     A: Marker,
//     D: Marker,
// {
//     buf: *mut u8, //[u8; 16],
//     phantom: std::marker::PhantomData<(A, D)>,
// }
//
// pub trait NiosLegacy {
//     type AddressType; // Placeholder for concrete type
//     type DataType; // Placeholder for concrete type
//     fn new() -> Self;
//     fn set(&mut self);
//     fn from_vec(v: Vec<u8>) -> Self;
//     fn into_vec(self) -> Vec<u8>;
//     fn reuse(v: Vec<u8>) -> Self;
//
//     fn magic(&self) -> u8;
//     fn configuration_byte(&mut self) -> u8;
//     fn addr(&self) -> u8;
//     fn data(&self) -> u8;
//
//     fn set_magic(&mut self, magic: u8) -> &mut Self;
//     fn set_configuration_byte(&mut self, configuration_byte: u8) -> &mut Self;
//     fn set_addr(&mut self, addr: u8) -> &mut Self;
//     fn set_data(&mut self, data: u8) -> &mut Self;
// }
//
// impl Debug for NiosPktLegacy<u8, u8> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut s = String::from("NIOS Legacy ");
//         // if self.write() {
//         //     s = s.add("WRITE ")
//         // }
//         // if self.read() {
//         //     s = s.add("READ ")
//         // }
//
//         f.debug_struct(s.as_str())
//             .field("magic", &self.magic())
//             //.field("target", &self.target_id())
//             //.field("flags", &self.flags())
//             //.field("write", &self.write())
//             //.field("success", &self.success())
//             //.field("addr", &self.addr())
//             //.field("data", &self.data())
//             .finish()
//         //.finish_non_exhaustive()
//     }
// }
//
// unsafe impl Send for NiosPktLegacy<u8, u8> {}
// unsafe impl Sync for NiosPktLegacy<u8, u8> {}
//
// impl<A, D> Drop for NiosPktLegacy<A, D>
// where
//     A: Marker,
//     D: Marker,
// {
//     fn drop(&mut self) {
//         unsafe { drop(Vec::from_raw_parts(self.buf, 16, 16)) }
//     }
// }
//
// impl NiosLegacy for NiosPktLegacy<u8, u8> {
//     type AddressType = u8;
//     type DataType = u8;
//
//     fn new() -> Self {
//         //let buf = Vec::<u8>::from([0u8; 16]);
//         let mut pkt = Self::from_vec(Vec::<u8>::from([0u8; 16]));
//         pkt.set();
//         pkt
//     }
//
//     fn set(&mut self) {
//         //let v = self.into_vec();
//         //let mut pkt = Self::reuse(v);
//         //let v = unsafe { Vec::<u8>::from_raw_parts(self.buf, 16, 16) };
//         //let mut v = ManuallyDrop::new(v);
//         // let mut pkt = Self {
//         //     buf: self.buf, //v.as_mut_ptr(),
//         //     phantom: std::marker::PhantomData,
//         // };
//         //println!("{:x?}", pkt.set_magic(0x41));
//         //println!("{:x?}", pkt);
//         let magic = NIOS_PKT_LEGACY_MAGIC;
//         self.set_magic(magic);
//         self.set_configuration_byte(NIOS_PKT_LEGACY_MODE_DIR_READ | 1);
//         self.set_addr(NIOS_PKT_LEGACY_PIO_ADDR_FPGA_VERSION);
//         self.set_data(0xff);
//     }
//
//     fn from_vec(v: Vec<u8>) -> Self {
//         let mut v = ManuallyDrop::new(v);
//         Self {
//             buf: v.as_mut_ptr(),
//             phantom: Default::default(),
//         }
//     }
//
//     fn into_vec(self) -> Vec<u8> {
//         let s = ManuallyDrop::new(self);
//         unsafe { Vec::<u8>::from_raw_parts(s.buf, 16, 16) }
//     }
//
//     fn reuse(v: Vec<u8>) -> Self {
//         let mut v = ManuallyDrop::new(v);
//         v.clear();
//         v.reserve_exact(16);
//         Self {
//             buf: v.as_mut_ptr(),
//             phantom: Default::default(),
//         }
//     }
//
//     fn magic(&self) -> u8 {
//         unsafe { self.buf.read() }
//     }
//
//     fn configuration_byte(&mut self) -> u8 {
//         unsafe { self.buf.add(1).read() }
//     }
//     fn addr(&self) -> u8 {
//         unsafe { self.buf.add(2).read() }
//     }
//
//     fn data(&self) -> u8 {
//         unsafe { self.buf.add(3).read() }
//     }
//
//     fn set_magic(&mut self, magic: u8) -> &mut Self {
//         unsafe {
//             self.buf.add(0).write(magic);
//         }
//         self
//     }
//
//     fn set_configuration_byte(&mut self, configuration_byte: u8) -> &mut Self {
//         unsafe {
//             self.buf.add(1).write(configuration_byte);
//         }
//         self
//     }
//
//     fn set_addr(&mut self, addr: u8) -> &mut Self {
//         unsafe {
//             self.buf.add(2).write(addr);
//         }
//         self
//     }
//
//     fn set_data(&mut self, data: u8) -> &mut Self {
//         unsafe {
//             self.buf.add(3).write(data);
//         }
//         self
//     }
// }
