#[cfg(target_os = "macos")]
mod symbol_macos;
#[cfg(target_os = "macos")]
use symbol_macos::SymbolParserImpl;
#[cfg(target_os = "linux")]
mod symbol_linux;
#[cfg(target_os = "linux")]
use symbol_linux::SymbolParserImpl;

use std::sync::{Mutex, Once};

use crate::code_cache::CodeCache;

static INSTANCE_ONCE: Once = Once::new();

pub struct SymbolParser {
    mutex: Mutex<()>,
    have_kernel_symbols: bool,
    symbol_impl: SymbolParserImpl,
}

impl SymbolParser {
    pub fn have_kernel_symbols(&self) -> bool {
        self.have_kernel_symbols
    }

    pub fn instance() -> &'static mut Self {
        static mut INSTANCE: Option<SymbolParser> = None;
        INSTANCE_ONCE.call_once(|| {
            unsafe {
                INSTANCE = Some(SymbolParser {
                    mutex: Mutex::new(()),
                    have_kernel_symbols: false,
                    symbol_impl: SymbolParserImpl::new(),
                })
            };
        });
        unsafe { INSTANCE.as_mut().unwrap() }
    }

    pub fn parse_libraries(&mut self, code_caches: &mut Vec<CodeCache>, parse_kernel: bool) {
        let _lock = self.mutex.lock();
        self.symbol_impl.parse_libraries(code_caches, parse_kernel);
    }
}
