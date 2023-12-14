use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader},
    ptr,
};

use crate::{code_cache::CodeCache, profiler::MAX_CODE_CACHE_ARRAY};

const SHN_UNDEF: u8 = 0;
const ET_EXEC: u16 = 2;

const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_PLTRELSZ: i64 = 2;
const DT_PLTGOT: i64 = 3;
const DT_HASH: i64 = 4;
const DT_STRTAB: i64 = 5;
const DT_SYMTAB: i64 = 6;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_RELAENT: i64 = 9;
const DT_STRSZ: i64 = 10;
const DT_SYMENT: i64 = 11;
const DT_INIT: i64 = 12;
const DT_FINI: i64 = 13;
const DT_SONAME: i64 = 14;
const DT_RPATH: i64 = 15;
const DT_SYMBOLIC: i64 = 16;
const DT_REL: i64 = 17;
const DT_RELSZ: i64 = 18;
const DT_RELENT: i64 = 19;
const DT_PLTREL: i64 = 20;
const DT_DEBUG: i64 = 21;
const DT_TEXTREL: i64 = 22;
const DT_JMPREL: i64 = 23;
const DT_RELACOUNT: i64 = 0x6ffffff9;
const DT_RELCOUNT: i64 = 0x6ffffffa;

#[cfg(target_pointer_width = "64")]
mod target_64 {
    pub const R_GLOB_DAT: u64 = 6;

    pub const ELF_R_TYPE_MASK: u64 = 0xffffffff;

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf64_Nhdr {
        pub n_namesz: libc::Elf64_Word,
        pub n_descsz: libc::Elf64_Word,
        pub n_type: libc::Elf64_Word,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf64_Rel {
        pub r_offset: libc::Elf64_Addr,
        pub r_info: libc::Elf64_Xword,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub union UnnamedDyn64 {
        pub d_val: libc::Elf64_Xword,
        pub d_ptr: libc::Elf64_Addr,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf64_Dyn {
        pub d_un: UnnamedDyn64,
        pub d_tag: libc::Elf64_Sxword,
    }

    pub const ELFCLASS_SUPPORTED: u8 = libc::ELFCLASS64;
    pub type ElfHeader = libc::Elf64_Ehdr;
    pub type ElfSection = libc::Elf64_Shdr;
    pub type ElfProgramHeader = libc::Elf64_Phdr;
    pub type ElfNote = Elf64_Nhdr;
    pub type ElfSymbol = libc::Elf64_Sym;
    pub type ElfRelocation = Elf64_Rel;
    pub type ElfDyn = Elf64_Dyn;
}

#[cfg(target_pointer_width = "64")]
use target_64::*;

#[cfg(target_pointer_width = "32")]
mod target_32 {
    pub const ELF_R_TYPE_MASK: u64 = 0xff;

    pub const R_GLOB_DAT: u64 = 6;
    type Elf32_Sword = u32;

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf32_Nhdr {
        pub n_namesz: libc::Elf32_Word,
        pub n_descsz: libc::Elf32_Word,
        pub n_type: libc::Elf32_Word,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf32_Rel {
        pub r_offset: libc::Elf32_Addr,
        pub r_info: libc::Elf32_Word,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    pub union UnnamedDyn32 {
        pub d_val: Elf32_Sword,
        pub d_ptr: libc::Elf32_Addr,
    }

    #[derive(Clone, Copy)]
    #[repr(C)]
    #[allow(non_camel_case_types)]
    pub struct Elf32_Dyn {
        d_un: UnnamedDyn,
        d_tag: Elf32_Sword,
    }

    pub const ELFCLASS_SUPPORTED: u8 = libc::ELFCLASS32;
    pub type ElfHeader = libc::Elf32_Ehdr;
    pub type ElfSection = libc::Elf32_Shdr;
    pub type ElfProgramHeader = libc::Elf32_Phdr;
    pub type ElfNote = Elf32_Nhdr;
    pub type ElfSymbol = libc::Elf32_Sym;
    pub type ElfRelocation = Elf32_Rel;
    pub type ElfDyn = libc::Elf32_Dyn;
}

#[cfg(target_pointer_width = "32")]
use target_32::*;

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
            Some(pos) => (&$split[0..pos], &$split[pos + 1..]),
            None => ($split, &[][..]),
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
        unsafe { Self::str_to_addr(self.end, 16) as _ }
    }

    pub fn addr(&self) -> *const i8 {
        unsafe { Self::str_to_addr(self.addr, 16) as _ }
    }

    pub fn offs(&self) -> u64 {
        unsafe { Self::str_to_addr(self.offs, 16) as _ }
    }

    pub fn inode(&self) -> u64 {
        unsafe { Self::str_to_addr(self.inode, 10) as _ }
    }

    pub fn is_empty_file(&self) -> bool {
        self.file.len() == 0
    }

    pub fn dev(&self) -> u64 {
        let dev = self.dev;
        let (maj, min) = split!(dev, b':');
        let maj = unsafe { Self::str_to_addr(maj, 16) };
        let min = unsafe { Self::str_to_addr(min, 16) };
        (maj << 8) | min
    }

    fn is_executable(&self) -> bool {
        self.perm[2] == b'x'
    }
}

pub(crate) struct SymbolParserImpl {
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

    pub fn parse_libraries(&mut self, code_caches: &mut Vec<CodeCache>, _parse_kernel: bool) {
        let mut map_file = match fs::OpenOptions::new().read(true).open("/proc/self/maps") {
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
                    image_end,
                );
                let inode = desc.inode();
                if inode != 0 {
                    // Do not parse the same executable twice, e.g. on Alpine Linux
                    if self.parsed_inode.insert(desc.dev() << 32 | inode) {
                        image_base = unsafe { image_base.offset(-(desc.offs() as isize)) };
                        if image_base >= last_readable_base {
                            // ElfParser::parse_program_headers()
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

struct ElfParser<'a, 'b> {
    cc: &'a mut CodeCache,
    base: *const i8,
    header: *const ElfHeader,
    file_name: &'b [u8],
    sections: *const i8,
    vaddr_diff: *const i8,
}

impl<'a, 'b> ElfParser<'a, 'b> {
    fn new(cc: &'a mut CodeCache, base: *const i8, addr: *const i8, file_name: &'b [u8]) -> Self {
        unsafe {
            let header = addr as *const ElfHeader;
            let sections = addr.add((*header).e_shoff as _);
            let vaddr_diff = ptr::null();
            Self {
                cc,
                base,
                header,
                sections,
                file_name,
                vaddr_diff,
            }
        }
    }

    #[inline(always)]
    unsafe fn valid_header(&self) -> bool {
        let elf_header = &*self.header;
        let ident = &elf_header.e_ident[..];
        ident[0] == 0x7f
            && ident[1] == b'E'
            && ident[2] == b'L'
            && ident[3] == b'F'
            && ident[4] == ELFCLASS_SUPPORTED
            && ident[5] == libc::ELFDATA2LSB
            && ident[6] == libc::EV_CURRENT as _
            && elf_header.e_shstrndx != SHN_UNDEF as _
    }

    #[inline(always)]
    fn set_text_base(&mut self, base: *const i8) {
        self.cc.set_text_base(base);
    }

    #[inline(always)]
    unsafe fn at_sectionhdr(&self, sec: *const ElfSection) -> *const i8 {
        (self.header as *const i8).offset((*sec).sh_offset as _)
    }

    #[inline(always)]
    unsafe fn at_programhdr(&self, pheader: *const ElfProgramHeader) -> *const i8 {
        if (*self.header).e_type == ET_EXEC {
            (*pheader).p_paddr as _
        } else {
            self.vaddr_diff.add((*pheader).p_paddr as _)
        }
    }

    unsafe fn parse_program_headers(cc: &'a mut CodeCache, base: *const i8, end: *const i8) {
        let mut elf_parser = ElfParser::new(cc, base, base, &[]);
        if elf_parser.valid_header() && base.offset((*elf_parser.header).e_phoff as _) < end {
            elf_parser.set_text_base(base);
            elf_parser.calc_virtual_local_address();
            elf_parser.parse_dynamic_section();
        }
    }

    unsafe fn parse_dynamic_section(&mut self) {
        macro_rules! dyn_ptr {
            ($p: expr) => {
                self.base.add($p as _)
            };
        }
        let dynamic = self.find_program_header(libc::PT_DYNAMIC);
        if dynamic.is_null() {
            return;
        }
        let mut got_start: *const *const () = ptr::null();
        let mut pltrelsz: isize = 0;
        let mut relsz: isize = 0;
        let mut relent: isize = 0;
        let mut relcount: isize = 0;
        let mut rel: *const i8 = ptr::null();
        let dyn_start = self.at_programhdr(dynamic);
        let dyn_end = dyn_start.add((*dynamic).p_memsz as _);
        let mut dy = dyn_start as *const ElfDyn;
        while dy < dyn_end as *const _ {
            match (*dy).d_tag {
                DT_PLTGOT => got_start = (dyn_ptr!((*dy).d_un.d_ptr) as *const *const ()).add(3),
                DT_PLTRELSZ => pltrelsz = (*dy).d_un.d_val as _,
                DT_RELA | DT_REL => rel = dyn_ptr!((*dy).d_un.d_ptr) as _,
                DT_RELASZ | DT_RELSZ => relsz = (*dy).d_un.d_val as _,
                DT_RELAENT | DT_RELENT => relent = (*dy).d_un.d_val as _,
                DT_RELACOUNT | DT_RELCOUNT => relcount = (*dy).d_un.d_val as _,
                _ => {}
            };
            dy = dy.add(1);
        }

        if relent != 0 {
            if pltrelsz != 0 && got_start.is_null() {
                self.cc.set_global_offset_table(
                    got_start as _,
                    got_start.add((pltrelsz / relent) as _) as _,
                    false,
                );
            } else if rel.is_null() && relsz != 0 {
                let mut min_addr: *const *const () = -1 as _;
                let mut max_addr: *const *const () = 0 as _;
                let mut offs = relcount * relent;
                while offs < relsz {
                    let r = rel.add(offs as _) as *const ElfRelocation;
                    if ((*r).r_info & ELF_R_TYPE_MASK) == R_GLOB_DAT {
                        let addr = self.base.add((*r).r_offset as _) as _;
                        if addr < min_addr {
                            min_addr = addr;
                        }
                        if addr > max_addr {
                            max_addr = addr;
                        }
                    }
                    offs += relent;
                }

                if got_start.is_null() {
                    got_start = min_addr;
                }
                if max_addr >= got_start {
                    self.cc
                        .set_global_offset_table(got_start as _, max_addr.add(1) as _, false);
                }
            }
        }
    }

    unsafe fn find_program_header(&self, typ: u32) -> *const ElfProgramHeader {
        let pheaders = (self.header as *const i8).offset((*self.header).e_phoff as _);
        for i in 0..(*self.header).e_phnum as isize {
            let pheader = &*(pheaders.offset(i * ((*self.header).e_phentsize as isize))
                as *const ElfProgramHeader);
            if pheader.p_type == typ {
                return pheader;
            }
        }
        return ptr::null();
    }

    unsafe fn calc_virtual_local_address(&mut self) {
        let pheaders = (self.header as *const i8).offset((*self.header).e_phoff as _);
        for i in 0..(*self.header).e_phnum as isize {
            let pheader = &*(pheaders.offset(i * ((*self.header).e_phentsize as isize))
                as *const ElfProgramHeader);
            if pheader.p_type == libc::PT_LOAD {
                self.vaddr_diff = self.base.offset(-(pheader.p_vaddr as isize));
            }
        }
        self.vaddr_diff = self.base;
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
        assert_eq!(desc.dev(), 0xfd << 8 | 00);
        assert_eq!(desc.inode(), 100694562);
        assert_eq!(desc.addr() as u64, 0x0060c000);
        assert_eq!(desc.end() as u64, 0x0060d000);
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
