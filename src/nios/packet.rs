// #![allow(unsafe_code)]
//
// use super::constants::*;
// //use std::fmt::Debug;
// use std::mem::ManuallyDrop;
// //use std::ops::Add;
//
// enum Direction {
//     RX = 0x0,
//     TX = 0x1,
// }
//
// const fn get_module(direction: Direction, channel: u8) -> u8 {
//     if direction as u8 == Direction::RX as u8 {
//         (channel << 1) | 0x0
//     } else {
//         (channel << 1) | 0x1
//     }
// }
//
// pub const BLADERF_MODULE_RX: u8 = get_module(Direction::RX, 0);
// pub const BLADERF_MODULE_TX: u8 = get_module(Direction::TX, 0);
//
// /*
//  * This file defines the Host <-> FPGA (NIOS II) packet formats for accesses
//  * to devices/blocks with 8-bit addresses and 8-bit data
//  *
//  *
//  *                              Request
//  *                      ----------------------
//  *
//  * +================+=========================================================+
//  * |  Byte offset   |                       Description                       |
//  * +================+=========================================================+
//  * |        0       | Magic Value                                             |
//  * +----------------+---------------------------------------------------------+
//  * |        1       | Target ID (Note 1)                                      |
//  * +----------------+---------------------------------------------------------+
//  * |        2       | Flags (Note 2)                                          |
//  * +----------------+---------------------------------------------------------+
//  * |        3       | Reserved. Set to 0x00.                                  |
//  * +----------------+---------------------------------------------------------+
//  * |        4       | 8-bit address                                           |
//  * +----------------+---------------------------------------------------------+
//  * |        5       | 8-bit data                                              |
//  * +----------------+---------------------------------------------------------+
//  * |      15:6      | Reserved. Set to 0.                                     |
//  * +----------------+---------------------------------------------------------+
//  *
//  *
//  *                              Response
//  *                      ----------------------
//  *
//  * The response packet contains the same information as the request.
//  * A status flag will be set if the operation completed successfully.
//  *
//  * In the case of a read request, the data field will contain the read data, if
//  * the read succeeded.
//  *
//  * (Note 1)
//  *  The "Target ID" refers to the peripheral, device, or block to access.
//  *  See the NIOS_PKT_8x8_TARGET_* values.
//  *
//  * (Note 2)
//  *  The flags are defined as follows:
//  *
//  *    +================+========================+
//  *    |      Bit(s)    |         Value          |
//  *    +================+========================+
//  *    |       7:2      | Reserved. Set to 0.    |
//  *    +----------------+------------------------+
//  *    |                | Status. Only used in   |
//  *    |                | response packet.       |
//  *    |                | Ignored in request.    |
//  *    |        1       |                        |
//  *    |                |   1 = Success          |
//  *    |                |   0 = Failure          |
//  *    +----------------+------------------------+
//  *    |        0       |   0 = Read operation   |
//  *    |                |   1 = Write operation  |
//  *    +----------------+------------------------+
//  *
//  */
//
// pub struct Packet {
//     buf: *mut u8,
// }
// impl Packet {
//     pub fn as_mut_ptr(&mut self) -> *mut u8 {
//         self.buf
//     }
//
//     pub fn from_vec(v: Vec<u8>) -> Self {
//         let mut v = ManuallyDrop::new(v);
//         Self {
//             buf: v.as_mut_ptr(),
//         }
//     }
//
//     pub fn new() -> Self {
//         Self::from_vec(Vec::<u8>::from([0u8; 16]))
//     }
//
//     pub fn into_vec(self) -> Vec<u8> {
//         let mut s = ManuallyDrop::new(self);
//         unsafe { Vec::<u8>::from_raw_parts(s.as_mut_ptr(), 16, 16) }
//     }
//
//     pub fn reuse(v: Vec<u8>) -> Self {
//         let mut v = ManuallyDrop::new(v);
//         v.clear();
//         v.reserve_exact(16);
//         Self {
//             buf: v.as_mut_ptr(),
//         }
//     }
// }
//
// impl Drop for Packet {
//     fn drop(&mut self) {
//         unsafe { drop(Vec::from_raw_parts(self.buf, 16, 16)) }
//     }
// }
//
// pub struct NiosPacket {
//     pub pkt: Packet,
// }
//
// impl NiosPacket {
//     pub fn new() -> Self {
//         Self { pkt: Packet::new() }
//     }
//
//     pub fn magic(&self) -> u8 {
//         unsafe { self.pkt.buf.read() }
//     }
//     pub fn target_id(&self) -> u8 {
//         unsafe { self.pkt.buf.add(NIOS_PKT_IDX_TARGET_ID).read() }
//     }
//
//     pub fn flags(&self) -> u8 {
//         unsafe { self.pkt.buf.add(NIOS_PKT_IDX_FLAGS).read() }
//     }
//
//     pub fn is_write(&self) -> bool {
//         unsafe { (self.pkt.buf.add(NIOS_PKT_IDX_FLAGS).read() & NIOS_PKT_FLAG_WRITE) != 0 }
//     }
//
//     pub fn set_magic(&mut self, magic: u8) -> &mut Self {
//         unsafe {
//             self.pkt.buf.add(NIOS_PKT_IDX_MAGIC).write(magic);
//         }
//         self
//     }
//     pub fn set_target_id(&mut self, target_id: u8) -> &mut Self {
//         unsafe {
//             self.pkt.buf.add(NIOS_PKT_IDX_TARGET_ID).write(target_id);
//         }
//         self
//     }
//     pub fn set_flag(&mut self, flag: u8) -> &mut Self {
//         unsafe {
//             let flags = self.pkt.buf.add(NIOS_PKT_IDX_FLAGS).read();
//             self.pkt.buf.add(NIOS_PKT_IDX_FLAGS).write(flags | flag);
//         }
//         self
//     }
//     pub fn set_flags(&mut self, flags: u8) -> &mut Self {
//         unsafe {
//             self.pkt.buf.add(NIOS_PKT_IDX_FLAGS).write(flags);
//         }
//         self
//     }
// }
//
// pub struct NiosPacket8x8 {
//     pub pkt: NiosPacket,
// }
//
// impl NiosPacket8x8 {
//     pub fn new() -> Self {
//         let mut pkt = NiosPacket::new();
//         pkt.set_magic(NIOS_PKT_8X8_MAGIC);
//         Self { pkt }
//     }
//
//     pub fn set_addr(&mut self, addr: u8) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR)
//                 .copy_from(addr.to_le_bytes().as_ptr(), ADDR_SIZE)
//         }
//         self
//     }
//     pub fn set_data(&mut self, data: u8) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//             const DATA_SIZE: usize = size_of::<u8>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR + ADDR_SIZE)
//                 .copy_from(data.to_le_bytes().as_ptr(), DATA_SIZE)
//         }
//         self
//     }
// }
//
// pub struct NiosPacket8x16 {
//     pub pkt: NiosPacket,
// }
//
// impl NiosPacket8x16 {
//     pub fn new() -> Self {
//         let mut pkt = NiosPacket::new();
//         pkt.set_magic(NIOS_PKT_8X16_MAGIC);
//         Self { pkt }
//     }
//
//     pub fn set_addr(&mut self, addr: u8) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR)
//                 .copy_from(addr.to_le_bytes().as_ptr(), ADDR_SIZE)
//         }
//         self
//     }
//     pub fn set_data(&mut self, data: u16) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//             const DATA_SIZE: usize = size_of::<u16>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR + ADDR_SIZE)
//                 .copy_from(data.to_le_bytes().as_ptr(), DATA_SIZE)
//         }
//         self
//     }
// }
//
// pub struct NiosPacket8x32 {
//     pub pkt: NiosPacket,
// }
//
// impl NiosPacket8x32 {
//     pub fn new() -> Self {
//         let mut pkt = NiosPacket::new();
//         pkt.set_magic(NIOS_PKT_8X32_MAGIC);
//         Self { pkt }
//     }
//
//     pub fn set_addr(&mut self, addr: u8) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR)
//                 .copy_from(addr.to_le_bytes().as_ptr(), ADDR_SIZE)
//         }
//         self
//     }
//     pub fn set_data(&mut self, data: u32) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//             const DATA_SIZE: usize = size_of::<u32>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR + ADDR_SIZE)
//                 .copy_from(data.to_le_bytes().as_ptr(), DATA_SIZE)
//         }
//         self
//     }
// }
//
// pub struct NiosPacket8x64 {
//     pub pkt: NiosPacket,
// }
//
// impl NiosPacket8x64 {
//     pub fn new() -> Self {
//         let mut pkt = NiosPacket::new();
//         pkt.set_magic(NIOS_PKT_8X64_MAGIC);
//         Self { pkt }
//     }
//
//     pub fn set_addr(&mut self, addr: u8) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR)
//                 .copy_from(addr.to_le_bytes().as_ptr(), ADDR_SIZE)
//         }
//         self
//     }
//     pub fn set_data(&mut self, data: u64) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u8>();
//             const DATA_SIZE: usize = size_of::<u64>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR + ADDR_SIZE)
//                 .copy_from(data.to_le_bytes().as_ptr(), DATA_SIZE)
//         }
//         self
//     }
// }
//
// pub struct NiosPacket32x32 {
//     pub pkt: NiosPacket,
// }
//
// impl NiosPacket32x32 {
//     pub fn new() -> Self {
//         let mut pkt = NiosPacket::new();
//         pkt.set_magic(NIOS_PKT_32X32_MAGIC);
//         Self { pkt }
//     }
//
//     pub fn set_addr(&mut self, addr: u32) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u32>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR)
//                 .copy_from(addr.to_le_bytes().as_ptr(), ADDR_SIZE)
//         }
//         self
//     }
//     pub fn set_data(&mut self, data: u32) -> &mut Self {
//         unsafe {
//             const ADDR_SIZE: usize = size_of::<u32>();
//             const DATA_SIZE: usize = size_of::<u32>();
//
//             self.pkt
//                 .pkt
//                 .buf
//                 .add(NIOS_PKT_IDX_ADDR + ADDR_SIZE)
//                 .copy_from(data.to_le_bytes().as_ptr(), DATA_SIZE)
//         }
//         self
//     }
// }
//
// // https://stackoverflow.com/questions/78395612/how-to-enforce-generic-parameter-to-be-of-type-u8-u16-u32-or-u64-in-rust
// // https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/
// // trait Marker {}
// //
// // impl Marker for u8 {}
// // impl Marker for u16 {}
// // impl Marker for u32 {}
// // impl Marker for u64 {}
// //
// // pub struct NiosPkt<A, D>
// // where
// //     A: Marker,
// //     D: Marker,
// // {
// //     buf: *mut u8,
// //     phantom: std::marker::PhantomData<(A, D)>,
// // }
// //
// // impl<A, D> NiosPkt<A, D>
// // where
// //     A: Marker + Sized,
// //     D: Marker + Sized,
// // {
// //     pub fn new(target_id: u8, flags: u8, addr: A, data: D) -> Self {
// //         let mut pkt = Self::from_vec(Vec::<u8>::from([0u8; 16]));
// //         pkt.set(target_id, flags, addr, data);
// //         pkt
// //     }
// //
// //     pub fn set(&mut self, target_id: u8, flags: u8, addr: A, data: D) {
// //         let magic = match (size_of::<A>(), size_of::<D>()) {
// //             (1, 1) => NIOS_PKT_8X8_MAGIC,
// //             (1, 2) => NIOS_PKT_8X16_MAGIC,
// //             (1, 4) => NIOS_PKT_8X32_MAGIC,
// //             (1, 8) => NIOS_PKT_8X64_MAGIC,
// //             (2, 8) => NIOS_PKT_16X64_MAGIC,
// //             (4, 4) => NIOS_PKT_32X32_MAGIC,
// //             _ => panic!("Wrong type sizes for NIOS packet"),
// //         };
// //         self.set_magic(magic);
// //         self.set_target_id(target_id);
// //         self.set_flags(flags);
// //         self.set_addr(addr);
// //         self.set_data(data);
// //     }
// //
// //     pub fn from_vec(v: Vec<u8>) -> Self {
// //         let mut v = ManuallyDrop::new(v);
// //         Self {
// //             buf: v.as_mut_ptr(),
// //             phantom: Default::default(),
// //         }
// //     }
// //
// //     pub fn into_vec(self) -> Vec<u8> {
// //         let s = ManuallyDrop::new(self);
// //         unsafe { Vec::<u8>::from_raw_parts(s.buf, 16, 16) }
// //     }
// //
// //     pub fn reuse(v: Vec<u8>) -> Self {
// //         let mut v = ManuallyDrop::new(v);
// //         v.clear();
// //         v.reserve_exact(16);
// //         Self {
// //             buf: v.as_mut_ptr(),
// //             phantom: Default::default(),
// //         }
// //     }
// //
// //     pub fn magic(&self) -> u8 {
// //         unsafe { self.buf.read() }
// //     }
// //     pub fn target_id(&self) -> u8 {
// //         unsafe { self.buf.add(NIOS_PKT_IDX_TARGET_ID).read() }
// //     }
// //
// //     pub fn flags(&self) -> u8 {
// //         unsafe { self.buf.add(NIOS_PKT_IDX_FLAGS).read() }
// //     }
// //
// //     pub fn is_write(&self) -> bool {
// //         unsafe { (self.buf.add(NIOS_PKT_IDX_FLAGS).read() & NIOS_PKT_FLAG_WRITE) != 0 }
// //     }
// //
// //     pub fn set_magic(&mut self, magic: u8) -> &mut Self {
// //         unsafe {
// //             self.buf.add(NIOS_PKT_IDX_MAGIC).write(magic);
// //         }
// //         self
// //     }
// //     pub fn set_target_id(&mut self, target_id: u8) -> &mut Self {
// //         unsafe {
// //             self.buf.add(NIOS_PKT_IDX_TARGET_ID).write(target_id);
// //         }
// //         self
// //     }
// //     pub fn set_flag(&mut self, flag: u8) -> &mut Self {
// //         unsafe {
// //             let flags = self.buf.add(NIOS_PKT_IDX_FLAGS).read();
// //             self.buf.add(NIOS_PKT_IDX_FLAGS).write(flags | flag);
// //         }
// //         self
// //     }
// //     pub fn set_flags(&mut self, flags: u8) -> &mut Self {
// //         unsafe {
// //             self.buf.add(NIOS_PKT_IDX_FLAGS).write(flags);
// //         }
// //         self
// //     }
// //
// //     pub fn set_addr(&mut self, addr: A) -> &mut Self {
// //         unsafe {
// //             let addr_bytes = std::mem::transmute::<*const A, [u8; size_of::<*const A>()]>(addr);
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR)
// //                 .copy_from(addr_bytes.as_ptr(), size_of::<A>())
// //         }
// //         self
// //     }
// //     pub fn set_data(&mut self, data: D) -> &mut Self {
// //         unsafe {
// //             let size = size_of::<D>();
// //             let data_bytes = std::mem::transmute::<D, [u8; size]>(data);
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR + size_of::<A>())
// //                 .copy_from(data_bytes.as_ptr(), size_of::<D>())
// //         }
// //         self
// //     }
// // }
// //
// // pub trait Packet {
// //
// // }
// // pub trait Nios : Packet {
// //     type AddressType; // Placeholder for concrete type
// //     type DataType; // Placeholder for concrete type
// //     fn addr(&self) -> Self::AddressType;
// //     fn data(&self) -> Self::DataType;
// // }
// //
// // impl Nios for NiosPkt<u8, u8> {
// //     type AddressType = u8;
// //     type DataType = u8;
// //
// //     fn addr(&self) -> Self::AddressType {
// //         let mut bytes = [0u8; size_of::<Self::AddressType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR)
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::AddressType>())
// //         }
// //         Self::AddressType::from_le_bytes(bytes)
// //     }
// //     fn data(&self) -> Self::DataType {
// //         let mut bytes = [0u8; size_of::<Self::DataType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR + size_of::<Self::AddressType>())
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::DataType>())
// //         }
// //         Self::DataType::from_le_bytes(bytes)
// //     }
// // }
// //
// // impl Nios for NiosPkt<u8, u16> {
// //     type AddressType = u8;
// //     type DataType = u16;
// //
// //     fn addr(&self) -> Self::AddressType {
// //         let mut bytes = [0u8; size_of::<Self::AddressType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR)
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::AddressType>())
// //         }
// //         Self::AddressType::from_le_bytes(bytes)
// //     }
// //     fn data(&self) -> Self::DataType {
// //         let mut bytes = [0u8; size_of::<Self::DataType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR + size_of::<Self::AddressType>())
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::DataType>())
// //         }
// //         Self::DataType::from_le_bytes(bytes)
// //     }
// // }
// //
// // impl Nios for NiosPkt<u8, u32> {
// //     type AddressType = u8;
// //     type DataType = u32;
// //
// //     fn addr(&self) -> Self::AddressType {
// //         let mut bytes = [0u8; size_of::<Self::AddressType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR)
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::AddressType>())
// //         }
// //         Self::AddressType::from_le_bytes(bytes)
// //     }
// //     fn data(&self) -> Self::DataType {
// //         let mut bytes = [0u8; size_of::<Self::DataType>()];
// //         unsafe {
// //             self.buf
// //                 .add(NIOS_PKT_IDX_ADDR + size_of::<Self::AddressType>())
// //                 .copy_to(bytes.as_mut_ptr(), size_of::<Self::DataType>())
// //         }
// //         Self::DataType::from_le_bytes(bytes)
// //     }
// // }
// //
// // impl<A, D> Debug for NiosPkt<A, D>
// // where
// //     A: Marker + Sized,
// //     D: Marker + Sized,
// // {
// //     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
// //         let mut s = String::from("NIOS II ");
// //         if self.is_write() {
// //             s = s.add("WRITE ")
// //         } else {
// //             s = s.add("READ ")
// //         }
// //
// //         f.debug_struct(s.as_str())
// //             .field("magic", &self.magic())
// //             .field("target", &self.target_id())
// //             .field("flags", &self.flags())
// //             //.field("write", &self.write())
// //             //.field("success", &self.success())
// //             //.field("addr", &self.addr())
// //             //.field("data", &self.data())
// //             .finish()
// //         //.finish_non_exhaustive()
// //     }
// // }
// //
// // unsafe impl<A, D> Send for NiosPkt<A, D>
// // where
// //     A: Marker + Sized,
// //     D: Marker + Sized,
// // {
// // }
// // unsafe impl<A, D> Sync for NiosPkt<A, D>
// // where
// //     A: Marker + Sized,
// //     D: Marker + Sized,
// // {
// // }
// //
// // impl<A, D> Drop for NiosPkt<A, D>
// // where
// //     A: Marker,
// //     D: Marker,
// // {
// //     fn drop(&mut self) {
// //         unsafe { drop(Vec::from_raw_parts(self.buf, 16, 16)) }
// //     }
// // }
