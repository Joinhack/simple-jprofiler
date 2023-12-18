use std::ffi::CStr;


pub struct NMethod{
    inner: *const i8,
    nmethod_name_offset: i32,
}

impl NMethod {
    pub fn new(inner: *const i8, nmethod_name_offset: i32) -> Self {
        Self {
            inner,
            nmethod_name_offset
        }
    }

    #[inline(always)]
    pub unsafe fn at(&self, pos: isize) -> *const i8 {
        self.inner.offset(pos)
    }

    pub unsafe fn name(&self) -> *const i8 {
        *(self.at(self.nmethod_name_offset as _) as *const *const i8)
    }

    pub unsafe fn name_str(&self) -> &str {
        let name = CStr::from_ptr(self.name());
        std::str::from_utf8_unchecked(name.to_bytes())
    }

}