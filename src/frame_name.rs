use std::{sync::Mutex, collections::HashMap, ptr};
use std::ffi::CStr;

use cpp_demangle::Symbol;

use crate::{
    profiler::ThreadInfo, 
    vm::{
        JVMPICallFrame, BCI_THREADID, BCI_NATIVE_FRAME
    }, 
    code_cache::CodeBlob, 
    jvmti_native::{jmethodID, jclass}, 
    get_vm, cstr_2_str
};

pub struct FrameName<'a> {
    threads_pool: &'a Mutex<HashMap<u64, ThreadInfo>>,
    name: Vec<u8>
}

impl<'a> FrameName<'a> {
    pub fn new(threads_pool: &'a Mutex<HashMap<u64, ThreadInfo>>) -> Self {
        Self {
            threads_pool,
            name: Vec::new(),
        }
    }

    unsafe fn java_method_name(&mut self, method_id: jmethodID) -> Option<()> {
        let jvmti = get_vm().jvmti();
        let mut method_name_ptr = ptr::null_mut();
        let mut method_sig_ptr = ptr::null_mut();
        let mut class_sig_ptr = ptr::null_mut();
        let mut class: jclass = ptr::null_mut();
        if 0 == jvmti.get_method_name(method_id, &mut method_name_ptr, &mut method_sig_ptr, ptr::null_mut())? {
            if 0 == jvmti.get_method_declaring_class(method_id, &mut class)? {
                if 0 == jvmti.get_class_signature(class, &mut class_sig_ptr, ptr::null_mut())? {
                    let class_sig = cstr_2_str!(class_sig_ptr);
                    let method_name = cstr_2_str!(method_name_ptr);
                    //trim the class Ljava/lang/String; 
                    self.java_class_name(&class_sig[1..class_sig.len() - 1].as_bytes());
                    self.name.push(b'.');
                    self.name.extend_from_slice(&method_name.as_bytes());
                    let method_sig = cstr_2_str!(method_sig_ptr);
                    self.name.extend_from_slice(method_sig.as_bytes());
                }
            }
        }
        jvmti.deallocate(method_name_ptr as _);
        jvmti.deallocate(method_sig_ptr as _);
        jvmti.deallocate(class_sig_ptr as _);
        Some(())
    }

    fn java_class_name(&mut self, class: &[u8]) {
        let mut array_dimension = 0;
        while class[array_dimension] == b'[' {
            array_dimension += 1;
        }
        if array_dimension == 0 {
            self.name.extend_from_slice(class);
        } else {
            match class[array_dimension] {
                b'B' => self.name.extend_from_slice(b"byte"),
                b'C' => self.name.extend_from_slice(b"char"),
                b'I' => self.name.extend_from_slice(b"int"),
                b'J' => self.name.extend_from_slice(b"long"),
                b'S' => self.name.extend_from_slice(b"short"),
                b'Z' => self.name.extend_from_slice(b"boolean"),
                b'F' => self.name.extend_from_slice(b"short"),
                b'D' => self.name.extend_from_slice(b"double"),
                _ => self.name.extend_from_slice(&class[1..class.len() - array_dimension - 2]),
            }
        }
        for _ in 0..array_dimension {
            self.name.extend_from_slice(b"[]")
        }
        let name = &mut self.name;
        //replace the / to '.' like java/lang/String
        for i in 0..name.len() {
            if name[i] == b'/' && !name[i+1].is_ascii_digit() {
                name[i] = b'.';
            }
        }
    }

    fn decode_native_name(&mut self, name: &[u8]) {
        if let Ok(symbol) = Symbol::new(name) {
            let symbol_str = symbol.to_string();
            self.name.extend_from_slice(symbol_str.as_bytes());
        } else {
            self.name.extend_from_slice(name);
        }
    }

    pub fn name(&mut self, frame: &JVMPICallFrame) -> &str
    {
        self.name.truncate(0);
        match frame.bci {
            BCI_THREADID => {
                let pool = self.threads_pool.lock().unwrap();
                let tid = frame.method_id as u64;
                self.name.extend_from_slice(match pool.get(&tid) {
                    Some(ti) => ti.name.as_bytes(),
                    None => b"unkonwn thread",
                });
                
            }
            BCI_NATIVE_FRAME => {
                let code_blob: &CodeBlob = unsafe {&*(frame.method_id as *const CodeBlob)};
                let mname = code_blob.name_str().as_bytes();
                if mname[0] == b'_' && mname[1] == b'Z' {
                    self.decode_native_name(mname);
                } else {
                    self.name.extend_from_slice(mname);
                }
            }
            _ => {
                unsafe {
                    if let None = self.java_method_name(frame.method_id) {
                        self.name.extend_from_slice(b"[jvmtiError]");
                    }
                };
            }
        };
        unsafe {
            std::str::from_utf8_unchecked(&self.name)   
        }
    }
}