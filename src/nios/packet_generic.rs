#![allow(unsafe_code, dead_code)]

use crate::nios::constants::{
    NIOS_PKT_FLAG_WRITE, NIOS_PKT_IDX_FLAGS, NIOS_PKT_IDX_MAGIC, NIOS_PKT_IDX_TARGET_ID,
};
use std::mem::ManuallyDrop;

pub struct NiosPacket {
    buf: *mut u8,
}
impl NiosPacket {
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buf
    }

    pub fn from_vec(v: Vec<u8>) -> Self {
        let mut v = ManuallyDrop::new(v);
        Self {
            buf: v.as_mut_ptr(),
        }
    }

    pub fn new() -> Self {
        Self::from_vec(Vec::<u8>::from([0u8; 16]))
    }

    pub fn set(&mut self, magic: u8, target_id: u8, flags: u8) {
        self.set_magic(magic);
        self.set_target_id(target_id);
        self.set_flags(flags);
    }

    pub fn into_vec(self) -> Vec<u8> {
        let mut s = ManuallyDrop::new(self);
        unsafe { Vec::<u8>::from_raw_parts(s.as_mut_ptr(), 16, 16) }
    }

    pub fn reuse(v: Vec<u8>) -> Self {
        let mut v = ManuallyDrop::new(v);
        v.clear();
        if v.capacity() < 16 {
            v.resize(16, 0);
        }
        Self {
            buf: v.as_mut_ptr(),
        }
    }

    pub fn success(&self) -> bool {
        unsafe { (self.buf.add(NIOS_PKT_IDX_FLAGS).read() & 0x2) != 0 }
    }

    pub fn magic(&self) -> u8 {
        unsafe { self.buf.read() }
    }
    pub fn target_id(&self) -> u8 {
        unsafe { self.buf.add(NIOS_PKT_IDX_TARGET_ID).read() }
    }

    pub fn flags(&self) -> u8 {
        unsafe { self.buf.add(NIOS_PKT_IDX_FLAGS).read() }
    }

    pub fn is_write(&self) -> bool {
        unsafe { (self.buf.add(NIOS_PKT_IDX_FLAGS).read() & NIOS_PKT_FLAG_WRITE) != 0 }
    }

    pub fn set_magic(&mut self, magic: u8) -> &mut Self {
        unsafe {
            self.buf.add(NIOS_PKT_IDX_MAGIC).write(magic);
        }
        self
    }
    pub fn set_target_id(&mut self, target_id: u8) -> &mut Self {
        unsafe {
            self.buf.add(NIOS_PKT_IDX_TARGET_ID).write(target_id);
        }
        self
    }
    pub fn set_flag(&mut self, flag: u8) -> &mut Self {
        unsafe {
            let flags = self.buf.add(NIOS_PKT_IDX_FLAGS).read();
            self.set_flags(flags | flag)
        }
    }
    pub fn set_flags(&mut self, flags: u8) -> &mut Self {
        unsafe {
            self.buf.add(NIOS_PKT_IDX_FLAGS).write(flags);
        }
        self
    }
}

impl Drop for NiosPacket {
    fn drop(&mut self) {
        unsafe { drop(Vec::from_raw_parts(self.buf, 16, 16)) }
    }
}
