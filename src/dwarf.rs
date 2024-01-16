#![allow(unused)]
use std::{ops::Add, ptr};
const DW_STACK_SLOT: i32 = std::mem::size_of::<*const ()>() as _;

#[cfg(target_pointer_width = "64")]
mod target64 {
    pub const DWARF_SUPPORTED: bool = true;
    pub const DW_REG_FP: i32 = 6;
    pub const DW_REG_SP: i32 = 7;
    pub const DW_REG_PC: i32 = 8;
}

#[cfg(target_pointer_width = "64")]
use target64::*;

#[cfg(target_pointer_width = "32")]
mod target32 {
    pub const DWARF_SUPPORTED: bool = true;
    pub const DW_REG_FP: i32 = 5;
    pub const DW_REG_SP: i32 = 4;
    pub const DW_REG_PC: i32 = 8;
}

#[cfg(target_pointer_width = "32")]
use target32::*;

use crate::log_error;

static DEFAULT_FRAME: FrameDesc = FrameDesc {
    loc: 0,
    cfa: DW_REG_FP | (2 * DW_STACK_SLOT) << 8,
    fp_off: 0,
};

pub struct FrameDesc {
    pub loc: u32,
    pub cfa: i32,
    pub fp_off: i32,
}

struct DwarfParser {
    ptr: *const i8,
    name: *const i8,
    image_base: *const i8,
    code_align: u32,
    data_align: i32,
}

impl DwarfParser {
    pub unsafe fn new(name: *const i8, image_base: *const i8, eh_frame_hdr: *const i8) -> Self {
        let mut parser = Self {
            name,
            image_base,
            code_align: 0,
            data_align: 0,
            ptr: ptr::null(),
        };
        parser.parse(eh_frame_hdr);
        parser
    }

    /// parse the .eh_frame_hdr section
    /// https://refspecs.linuxbase.org/LSB_2.1.0/LSB-Embedded/LSB-Embedded/ehframehdr.html
    /// |Encoding | Field |
    /// |------ | ---------|
    /// |unsigned byte | version|
    /// |unsigned byte | eh_frame_ptr_enc|
    /// |unsigned byte | fde_count_enc|
    /// |unsigned byte | table_enc|
    /// |u32 | eh_frame_ptr|
    /// |u32 | fde_count|
    /// |  |binary search table|
    unsafe fn parse(&mut self, eh_frame_hdr: *const i8) {
        let version = *eh_frame_hdr;
        let eh_frame_ptr_enc = *eh_frame_hdr.add(1) as u8;
        let fde_count_enc = *eh_frame_hdr.add(2) as u8;
        let table_enc = *eh_frame_hdr.add(3) as u8;
        if version != 0x1
            || (eh_frame_ptr_enc & 0x7) != 0x3
            || (fde_count_enc & 0x7) != 0x3
            || (table_enc & 0xf7) != 0x33
        {
            log_error!("WARN: .eh_frame_hdr {version:#X} {eh_frame_ptr_enc:#X} {fde_count_enc:#X} {table_enc:#X}");
            return;
        }
        let fde_count = *(eh_frame_hdr.add(8) as *const u32);
        let table = *(eh_frame_hdr.add(16) as *const u32);
        for i in 0..fde_count {
            self.ptr = eh_frame_hdr.add(table.add(i * 2) as _);
            self.parse_fde();
        }
    }

    #[inline(always)]
    unsafe fn add<T>(&mut self, s: usize) -> *const T {
        let old_ptr = self.ptr;
        self.ptr = self.ptr.add(s);
        old_ptr as _
    }

    #[inline(always)]
    unsafe fn getu32(&mut self) -> u32 {
        *self.add(4)
    }

    #[inline(always)]
    unsafe fn getu16(&mut self) -> u16 {
        *self.add(2)
    }

    #[inline(always)]
    unsafe fn getu8(&mut self) -> u8 {
        *self.add(1)
    }

    #[inline(always)]
    unsafe fn get_ptr(&mut self) -> *const i8 {
        let old_ptr = self.ptr;
        old_ptr.add(*self.add(4))
    }

    unsafe fn get_leb(&mut self) -> u32 {
        let mut result = 0u32;
        let mut shift = 7;
        loop {
            let p = *self.ptr as u8;
            result |= (p as u32 & 0x7f) << shift;
            if p & 0x80 == 0 {
                break;
            }
            shift += 7;
            self.ptr = self.ptr.add(1);
        }
        result
    }

    unsafe fn get_sleb(&mut self) -> i32 {
        let mut result = 0i32;
        let mut shift = 7;
        loop {
            let p = *self.ptr as u8;
            result |= (p as i32 & 0x7f) << shift;
            if p & 0x80 == 0 {
                if (p & 0x40) != 0 {
                    shift += 7;
                    if shift < 32 {
                        result |= -1 << shift;
                    }
                }
                break;
            }
            shift += 7;
            self.ptr = self.ptr.add(1);
        }
        result
    }

    unsafe fn skip_leb(&mut self) {
        let mut b = *self.ptr as u8;
        while b & 0x80 != 0 {
            self.ptr = self.ptr.add(1);
            b = *self.ptr as u8;
        }
    }

    unsafe fn parse_cie(&mut self) {
        let cie_len = self.getu32();
        if cie_len == 0 || cie_len == 0xffffffff {
            return;
        }
        let cie_start = self.ptr;
        self.ptr = self.ptr.add(5);
        loop {
            if *self.ptr == 0 {
                break;
            }
            self.ptr = self.ptr.add(1);
        }
        self.code_align = self.get_leb();
        self.data_align = self.get_sleb();
        self.ptr = cie_start.add(cie_len as _);
    }

    unsafe fn parse_fde(&mut self) {
        let fde_len = self.getu32();
        if fde_len == 0 || fde_len == 0xffffffff {
            return;
        }
        let fde_start = self.ptr;
        let cie_off = self.getu32();
        self.ptr = fde_start.add(cie_off as _);
        self.parse_cie();
        self.ptr = fde_start.add(4);
        let range_start = self.get_ptr().offset_from(self.image_base);
        let range_len = self.getu32();
        self.ptr = self.ptr.add(self.get_leb() as _);
        todo!()
    }
}
