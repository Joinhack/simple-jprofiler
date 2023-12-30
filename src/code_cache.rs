use std::cmp::Ordering;
use std::ffi::CStr;
use std::ptr;
use std::rc::Rc;

const NO_MIN_ADDRESS: *const i8 = -1 as _;
const NO_MAX_ADDRESS: *const i8 = 0 as _;
const INITIAL_CODE_CACHE_CAPACITY: usize = 1024;

pub struct NativeFunc {
    lib_index: u16,
    mark: bool,
    name: Vec<u8>,
}

impl NativeFunc {
    pub fn create(name: *const i8, lib_index: u16) -> Self {
        let name = unsafe { CStr::from_ptr(name).to_bytes() };
        let name = name.into();
        Self {
            lib_index,
            mark: false,
            name,
        }
    }

    pub fn mark(&mut self) {
        self.mark = true;
    }

    pub fn is_mark(&self) -> bool {
        self.mark
    }

    #[inline(always)]
    pub fn name(&self) -> &[u8] {
        &self.name
    }

    #[inline(always)]
    pub fn name_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(&self.name) }
    }

    #[inline(always)]
    pub fn name_ptr(&self) -> *const u8 {
        self.name.as_ptr()
    }

    #[inline(always)]
    pub fn name_mut(&mut self) -> &mut [u8] {
        &mut self.name
    }
}

pub struct CodeBlob {
    name: NativeFunc,
    start: *const i8,
    end: *const i8,
}

impl CodeBlob {
    fn new(name: NativeFunc, start: *const i8, end: *const i8) -> Self {
        Self { name, start, end }
    }

    #[inline(always)]
    pub fn name_str(&self) -> &str {
        self.name.name_str()
    }

    #[inline(always)]
    pub fn name_ptr(&self) -> *const u8 {
        self.name.name_ptr()
    }

    fn cmp(&self, other: &Self) -> Ordering {
        if self.start < other.start {
            Ordering::Less
        } else if self.start > other.start {
            Ordering::Greater
        } else if self.end == other.end {
            Ordering::Equal
        } else {
            if self.end < other.end {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        }
    }
}

pub struct CodeCache {
    name: NativeFunc,
    lib_index: u16,
    pub(crate) min_address: *const i8,
    pub(crate) max_address: *const i8,
    text_base: *const i8,
    got_start: *const *const i8,
    got_end: *const *const i8,
    got_patchable: bool,
    debug_symbols: bool,
    blobs: Vec<CodeBlob>,
}

impl CodeCache {
    pub fn new(name: *const i8, lib_index: u16) -> Self {
        Self::new_with_address_range(name, lib_index, NO_MIN_ADDRESS, NO_MAX_ADDRESS)
    }

    pub fn find_symbol(&self, name: &[u8]) -> Option<*const i8> {
        self.blobs
            .iter()
            .find(|blob| blob.name.name() == name)
            .map(|blob| blob.start)
    }

    pub fn new_with_address_range(
        name: *const i8,
        lib_index: u16,
        min_address: *const i8,
        max_address: *const i8,
    ) -> Self {
        let name = NativeFunc::create(name, lib_index);
        Self {
            name,
            lib_index,
            min_address,
            max_address,
            text_base: ptr::null(),
            got_start: ptr::null(),
            got_end: ptr::null(),
            got_patchable: false,
            debug_symbols: false,
            blobs: Vec::with_capacity(INITIAL_CODE_CACHE_CAPACITY),
        }
    }

    pub fn set_debug_symbols(&mut self, b: bool) {
        self.debug_symbols = b;
    }

    pub fn add(
        &mut self,
        start: *const i8,
        length: usize,
        name: *const i8,
        is_update_bounds: bool,
    ) {
        let mut name = NativeFunc::create(name, self.lib_index);
        for val in name.name_mut() {
            if *val < b' ' {
                *val = b'?';
            }
        }
        let end = start.wrapping_add(length);
        self.blobs.push(CodeBlob::new(name, start, end));
        if is_update_bounds {
            self.update_bounds(start, end);
        }
    }

    #[inline(always)]
    pub fn code_blobs(&self) -> &[CodeBlob] {
        &self.blobs
    }

    #[inline(always)]
    pub fn name_str(&self) -> &str {
        self.name.name_str()
    }

    pub fn update_bounds(&mut self, start: *const i8, end: *const i8) {
        if start < self.min_address {
            self.min_address = start;
        }
        if end > self.max_address {
            self.max_address = end;
        }
    }

    pub fn sort(&mut self) {
        if self.blobs.len() == 0 {
            return;
        }
        self.blobs.sort_by(|o1, o2| o1.cmp(o2));
        if self.min_address == NO_MIN_ADDRESS {
            self.min_address = self.blobs[0].start;
        }
        if self.max_address == NO_MAX_ADDRESS {
            self.max_address = self.blobs[self.blobs.len() - 1].end;
        }
    }

    pub fn binary_search(&self, addr: *const i8) -> Option<&CodeBlob> {
        let low = match self.blobs.binary_search_by(|cb| {
            if cb.end <= addr {
                Ordering::Less
            } else if cb.start > addr {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }) {
            Ok(pos) => return self.blobs.get(pos),
            Err(low) => low,
        };
        if low > 0 && self.blobs[low - 1].start == self.blobs[low - 1].end || self.blobs[low - 1].end == addr {
            return self.blobs.get(low - 1);
        }
        None
    }

    #[inline(always)]
    pub fn set_text_base(&mut self, text_base: *const i8) {
        self.text_base = text_base;
    }

    pub fn find_symbol_prefix(&self, name: &[u8]) -> Option<*const i8> {
        self.blobs
            .iter()
            .find(|blob| blob.name.name().starts_with(name))
            .map(|blob| blob.start)
    }

    #[inline(always)]
    pub fn contains(&self, addr: *const i8) -> bool {
        addr >= self.min_address && addr < self.max_address
    }

    pub fn set_global_offset_table(
        &mut self,
        start: *const *const i8,
        end: *const *const i8,
        patchable: bool,
    ) {
        self.got_start = start;
        self.got_end = end;
        self.got_patchable = patchable;
    }
}

mod test {
    use super::*;
    use crate::c_str;

    #[test]
    fn test_sort() {
        let mut code_cache = CodeCache::new(c_str!("test") as _, 1);
        code_cache.add(120 as _, 10, c_str!("test1") as _, true);
        code_cache.add(100 as _, 10, c_str!("test1") as _, true);
        code_cache.add(140 as _, 10, c_str!("test1") as _, true);
        code_cache.sort();
        assert_eq!(code_cache.min_address, 100 as _);
        assert_eq!(code_cache.max_address, 150 as _);
    }
}
