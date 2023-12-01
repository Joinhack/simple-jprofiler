use std::{
    fs,
    ptr,
    io::{BufReader, BufRead},
    collections::HashSet
};

use crate::{code_cache::CodeCache, profiler::MAX_CODE_CACHE_ARRAY};



struct MemoryMapDesc<'a> {
    addr: &'a [u8],
    end: &'a [u8],
    perm: &'a [u8],
    offs: &'a [u8],
    dev: &'a [u8],
    inode: &'a [u8],
    file: &'a [u8],
}

macro_rules! split {
    ($split: ident, $s: expr) => {
        match $split.iter().position(|a| *a == $s) {
            Some(pos) => (&$split[0..pos], &$split[pos+1..]),
            None => ($split, &[][..])
        }
    };
}

impl<'a> MemoryMapDesc<'a> {
    fn parse(line: &'a [u8]) -> Self {
        
        let split = line;
        let (addr, split) = split!(split, b'-');
        let (end, split) = split!(split, b' ');
        let (perm, split) = split!(split, b' ');
        let (offs, split) = split!(split, b' ');
        let (dev, split) = split!(split, b' ');
        let (inode, split) = split!(split, b' ');
        let pos = split.iter().position(|s| *s != b' ');
        let file = match pos {
            Some(pos) => &split[pos..],
            None => &[],
        };
        Self { 
            addr,
            end,
            perm,
            offs,
            dev,
            inode,
            file,
         }
    }

    fn is_readable(&self) -> bool {
        self.perm[0] == b'r'
    }

    unsafe fn str_to_addr(b: &[u8], r: u32) -> u64 {
        let addr = std::str::from_utf8_unchecked(b);
        match u64::from_str_radix(addr, r) {
            Err(_) => 0,
            Ok(p) => p, 
        }
    }

    pub fn end(&self) -> *const i8 {
        unsafe {Self::str_to_addr(self.end, 16) as _}
    }

    pub fn addr(&self) -> *const i8 {
        unsafe {Self::str_to_addr(self.addr, 16) as _}
    }

    pub fn offs(&self) -> u64 {
        unsafe {Self::str_to_addr(self.offs, 16) as _}
    }

    pub fn inode(&self) -> u64 {
        unsafe {Self::str_to_addr(self.inode, 10) as _}
    }

    pub fn is_empty_file(&self) -> bool {
        self.file.len() == 0
    }

    pub fn dev(&self) -> u64 {
        let dev = self.dev;
        let (maj, min) = split!(dev, b':');
        let maj = unsafe {Self::str_to_addr(maj, 16)};
        let min = unsafe {Self::str_to_addr(min, 16)};
        (maj<<8) | min
    }


    fn is_executable(&self) -> bool {
        self.perm[2] == b'x'
    }
}

struct ElfParser {

}

impl ElfParser {
    
}

pub(crate)struct SymbolParserImpl {
    parsed_library: HashSet<u64>,
    parsed_inode: HashSet<u64>,
}

impl SymbolParserImpl {
    pub fn new() -> Self {
        Self { 
            parsed_library: HashSet::new(),
            parsed_inode: HashSet::new(),
        }
    }

    pub fn parse_libraries(&mut self, code_caches: &mut Vec<CodeCache>) {
        let mut map_file = match fs::OpenOptions::new()
            .read(true)
            .open("/proc/self/maps") {
                Ok(f) => BufReader::new(f),
                Err(_) => return,
            };
        let mut line = String::new();
        let mut image_end: *const i8 = ptr::null();
        let mut last_readable_base: *const i8 = ptr::null();

        while let Ok(n) = map_file.read_line(&mut line) {
            if n == 0 {
                break;
            }
            let desc = MemoryMapDesc::parse(&line.as_bytes());
            if !desc.is_readable() || desc.is_empty_file() {
                continue;
            }
            let mut image_base = desc.addr();
            if image_base == image_end {
                last_readable_base = image_base;
            }
            image_end = desc.end();

            if desc.is_executable() {
                // if already parsed the file, don't parse again.
                if !self.parsed_library.insert(image_base as _) {
                    continue;
                }

                let array_len = code_caches.len();
                if array_len >= MAX_CODE_CACHE_ARRAY as _ {
                    break;
                }
                let mut cc = CodeCache::new_with_address_range(
                    desc.file.as_ptr() as _, 
                    array_len as _, 
                    image_base, 
                    image_end
                );
                let inode = desc.inode();
                if inode != 0 {
                    // Do not parse the same executable twice, e.g. on Alpine Linux
                    if self.parsed_inode.insert(desc.dev() << 32 | inode) {
                        image_base = unsafe {image_base.offset(- (desc.offs() as isize))};
                        if image_base >= last_readable_base {
                            todo!()
                        }
                        todo!()
                    }
                } else if desc.file == b"[vdso]" {
                    todo!()
                }
            }
            line.truncate(0);
        }
    }

}

mod test {
    use super::*;
    #[test]
    fn test_memory_desc() {
        let line = b"0060c000-0060d000 rw-p 0000c000 fd:00 100694562                          /usr/bin/cat\0";
        let desc = MemoryMapDesc::parse(line);
        assert_eq!(desc.addr, b"0060c000");
        assert_eq!(desc.end, b"0060d000");
        assert_eq!(desc.perm, b"rw-p");
        assert_eq!(desc.offs, b"0000c000");
        assert_eq!(desc.dev, b"fd:00");
        assert_eq!(desc.inode, b"100694562");
        assert_eq!(desc.file, b"/usr/bin/cat\0");
        assert_eq!(desc.dev(), 0xfd<<8 | 00);
        assert_eq!(desc.inode(), 100694562);
        assert_eq!(desc.addr() as u64, 0x0060c000);
        assert_eq!(desc.end()as u64, 0x0060d000);
        assert_eq!(desc.is_executable(), false);
        assert_eq!(desc.is_readable(), true);
    }

    #[test]
    fn test_memory_desc_file() {
        let line = b"7fa750f39000-7fa750f3c000 ---p 00000000 00:00 0\0";
        let desc = MemoryMapDesc::parse(line);
        assert_eq!(desc.is_empty_file(), true);
    }
}