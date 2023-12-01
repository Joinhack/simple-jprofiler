use std::ptr;



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
        {
            let pos = $split.iter().position(|a| *a == $s).unwrap();
            let rs = &$split[0..pos];
            (rs, &$split[pos+1..])
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
        let pos = split.iter().position(|s| *s != b' ').unwrap();
        let file = &split[pos..];
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

mod test {
    use super::*;
    #[test]
    fn test_memory_desc() {
        let line = b"0060c000-0060d000 rw-p 0000c000 fd:00 100694562                          /usr/bin/cat";
        let desc = MemoryMapDesc::parse(line);
        assert_eq!(desc.addr, b"0060c000");
        assert_eq!(desc.end, b"0060d000");
        assert_eq!(desc.perm, b"rw-p");
        assert_eq!(desc.offs, b"0000c000");
        assert_eq!(desc.dev, b"fd:00");
        assert_eq!(desc.inode, b"100694562");
        assert_eq!(desc.file, b"/usr/bin/cat");
        assert_eq!(desc.dev(), 0xfd<<8 | 00);
        assert_eq!(desc.inode(), 100694562);
        assert_eq!(desc.addr() as u64, 0x0060c000);
        assert_eq!(desc.end()as u64, 0x0060d000);
        assert_eq!(desc.is_executable(), false);
        assert_eq!(desc.is_readable(), true);
    }
}