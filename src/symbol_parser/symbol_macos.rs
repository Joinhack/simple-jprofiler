use std::{
    mem,
    collections::HashSet, 
    ffi::CStr
};

use crate::{
    code_cache::CodeCache, 
    profiler::MAX_CODE_CACHE_ARRAY, 
    log_info
};

#[repr(C)]
#[allow(non_camel_case_types)]
struct section_64 { /* for 64-bit architectures */
	sectname: [i8; 16],	/* name of this section */
	segname: [i8; 16],	/* segment this section goes in */
	addr: u64,	/* memory address of this section */
	size: u64,		/* size in bytes of this section */
	offset: u32,	/* file offset of this section */
	align: u32,		/* section alignment (power of 2) */
	reloff: u32,		/* file offset of relocation entries */
	nreloc: u32,		/* number of relocation entries */
	flags: u32,		/* flags (section type and attributes)*/
	reserved1: u32,	/* reserved (for offset or index) */
	reserved2: u32,	/* reserved (for count or sizeof) */
	reserved3: u32,	/* reserved */
}

#[repr(C)]
#[allow(non_camel_case_types)]
struct symtab_command {
	cmd: u32,		/* LC_SYMTAB */
	cmdsize:u32,	/* sizeof(struct symtab_command) */
	symoff:u32,		/* symbol table offset */
	nsyms:u32,		/* number of symbol table entries */
	stroff:u32,		/* string table offset */
	strsize:u32,	/* string table size in bytes */
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct nlist_64 {
    pub n_un: C2RustUnnamed,
    pub n_type: u8,
    pub n_sect: u8,
    pub n_desc: u16,
    pub n_value: u64,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub n_strx: u32,
}

const LC_SYMTAB: u32 = 0x2;

const UNDEFINED: *const i8 = -1 as _;

const SEG_TEXT: &[u8] = b"__TEXT";
const SEG_LINKEDIT: &[u8] = b"__LINKEDIT";
const SEG_DATA: &[u8] = b"__DATA";
const SEC_SYMBOL_PTR: &[u8] = b"__la_symbol_ptr";

#[repr(C)]
union MachHeader<'a> {
    image_base: &'a libc::mach_header,
    image_base64: &'a libc::mach_header_64,
}

impl<'a> MachHeader<'a> {
    fn new(image_base: *const libc::mach_header) -> Self {
        Self {
            image_base: unsafe {
                &(*image_base)
            }
        }
    }

    #[inline(always)]
    unsafe fn raw(&self) -> *const i8 {
        self.image_base64 as *const _ as _
    }

    #[inline(always)]
    unsafe fn image_base64(&self) -> &libc::mach_header_64 {
        self.image_base64
    }

    #[inline(always)]
    unsafe fn raw_base64_offset<T>(&self, off: isize) -> *const T {
        let base64_ptr = self.image_base64 as *const libc::mach_header_64;
        return base64_ptr.offset(off) as *const T;
    }

    #[inline(always)]
    unsafe fn is_64(&self) -> bool {
        self.image_base.magic == libc::MH_MAGIC_64
    }
}

struct MachObjectParser<'a, 'b> {
    cc: &'a mut CodeCache,
    image_base: MachHeader<'b>,
}

#[inline(always)]
unsafe fn is_slice_eq(s:&[i8], c:&[u8]) -> bool {
    let sli: &[i8] = std::mem::transmute(c);
    sli == &s[0..sli.len()]
}

impl<'a, 'b> MachObjectParser<'a, 'b> {
    fn new(cc: &'a mut CodeCache, image_base: MachHeader<'b>) -> Self {
        Self {
            cc,
            image_base
        }
    }

    #[inline(always)]
    unsafe fn offset<T>(p: *const i8, size: isize) ->  *const T {
        p.offset(size) as *const T
    }

    #[inline(always)]
    unsafe fn cast<T, U>(f: &U) -> &T {
        &*(f as *const U as *const T)
    }

    unsafe fn find_global_offset_table(&mut self, sc: &libc::segment_command_64) {
        let sc_size = mem::size_of::<libc::segment_command_64>() as _;
        let mut section: &section_64 =  &*Self::offset(sc as *const _ as _, sc_size);
        for _ in 0..sc.nsects {
            if is_slice_eq(&section.sectname, SEC_SYMBOL_PTR) {
                let got_start = Self::offset::<i8>(self.image_base.raw(), section.addr as _);
                self.cc.set_global_offset_table(got_start as _, got_start.add(section.size as _) as _, true);
                break;
            }
            section = &*(section as *const section_64).add(1)
        }
    }

    unsafe fn load_symbol(
        &mut self, 
        symtab: *const symtab_command,
        text_base: *const i8, 
        link_base: *const i8
    ) {
        let symtab = &*symtab;
        let mut sym: &nlist_64  = &*Self::offset(link_base, symtab.symoff as _);
        let str_table = Self::offset::<i8>(link_base, symtab.stroff as _);
        let mut debug_symbols = false;
        for _ in 0..symtab.nsyms {
            if sym.n_type & 0xee == 0x0e && sym.n_value != 0 {
                let addr = text_base.add(sym.n_value as _);
                let mut name = str_table.add(sym.n_un.n_strx as _);
                if *name == b'_' as _ {
                    name = name.add(1);
                }
                debug_symbols = true;
                
                self.cc.add(addr, 0, name, false);
            }
            sym = &*((sym as *const nlist_64).add(1));
        }
        self.cc.set_debug_symbols(debug_symbols);
    }

    unsafe fn parse(&mut self) -> bool {
        if !self.image_base.is_64() {
            return false
        }
        let header = self.image_base.image_base64();
        let mut lc: &libc::load_command = &*self.image_base.raw_base64_offset(1);
        let mut text_base = UNDEFINED;
        let mut link_base = UNDEFINED;
        for _ in 0..header.ncmds {
            match lc.cmd {
                libc::LC_SEGMENT_64 => {
                    let sc = Self::cast::<libc::segment_command_64, _>(lc);
                    if (sc.initprot & 0x4) != 0 {
                        if text_base == UNDEFINED || is_slice_eq(&sc.segname, SEG_TEXT) {
                            let image_base = self.image_base.raw();
                            text_base = Self::offset::<i8>(image_base, -(sc.vmaddr as isize));
                            self.cc.set_text_base(text_base);
                            self.cc.update_bounds(image_base, Self::offset::<i8>(image_base, sc.vmaddr as _));
                        } 
                    } else if (sc.initprot & 0x7) == 0x1 {
                        if link_base == UNDEFINED && is_slice_eq(&sc.segname, SEG_LINKEDIT) {
                            link_base = text_base.offset(sc.vmaddr as isize -  sc.fileoff as isize);
                        }
                    } else if sc.initprot & 0x2 != 0 {
                        if is_slice_eq(&sc.segname, SEG_DATA) {
                            self.find_global_offset_table(sc);
                        }
                    }
                }
                LC_SYMTAB => {
                    if text_base == UNDEFINED || link_base == UNDEFINED {
                        return false;
                    }
                    self.load_symbol(lc as *const _ as _, text_base, link_base);
                    break;
                }
                _ => {}
            };
            lc = &*Self::offset(lc as *const _ as _, lc.cmdsize as _);
            
        }
        true
    }
}

struct DlHandle {
    pub(crate) handle: *mut libc::c_void,
}

impl Drop for DlHandle {
    fn drop(&mut self) {
        unsafe {libc::dlclose(self.handle)};
    }
}

pub(crate)struct SymbolParserImpl {
    parsed: HashSet<*const i8>
}

impl SymbolParserImpl {
    pub fn new() -> Self {
        Self {
            parsed: HashSet::new()
        }
    }

    pub fn parse_libraries(&mut self, code_caches: &mut Vec<CodeCache>) {
        unsafe {
            let count = libc::_dyld_image_count();
            for i in 0..count {
                let image_base = libc::_dyld_get_image_header(i);
                //already parsed, contnue;
                if image_base.is_null() || !self.parsed.insert(image_base as _) {
                    continue;
                }

                let dll_name = libc::_dyld_get_image_name(i);
                let handle = libc::dlopen(dll_name, libc::RTLD_LAZY|libc::RTLD_NOLOAD);
                if handle.is_null() {
                    continue;
                }
                let _hanlde = DlHandle {handle};
                let array_len = code_caches.len();
                if array_len >= MAX_CODE_CACHE_ARRAY as _ {
                    break;
                }
                let mut cc = CodeCache::new(dll_name, array_len as _);
                let mach_header = MachHeader::new(image_base);
                let mut parser = MachObjectParser::new(&mut cc, mach_header);
                if !parser.parse() {
                    log_info!("WARNING: parse error {:?}", CStr::from_ptr(dll_name).to_str());
                }
                cc.sort();
                code_caches.push(cc);
            }
        }
    }
}