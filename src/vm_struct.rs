mod code_heap;
mod nmethod;
mod vmthread;
pub use code_heap::CodeHeap;
pub use nmethod::NMethod;
use std::{ffi::CStr, fmt::Display, ptr};

use libc::uintptr_t;

use crate::{
    code_cache::CodeCache, 
    jvmti_native::{jfieldID, jthread}, 
    get_vm_mut, 
    jvmti::JNIEnv, 
    c_str
};

use self::vmthread::VMThread;

pub struct VMStruct {
    klass_name_offset: i32,
    symbol_length_offset: i32,
    symbol_length_and_refcount_offset: i32,
    symbol_body_offset: i32,
    nmethod_name_offset: i32,
    nmethod_method_offset: i32,
    nmethod_entry_offset: i32,
    nmethod_state_offset: i32,
    nmethod_level_offset: i32,
    method_constmethod_offset: i32,
    method_code_offset: i32,
    constmethod_constants_offset: i32,
    constmethod_idnum_offset: i32,
    pool_holder_offset: i32,
    class_loader_data_offset: i32,
    methods_offset: i32,
    jmethod_ids_offset: i32,
    class_loader_data_next_offset: i32,
    klass_offset_addr: *const i32,
    thread_osthread_offset: i32,
    thread_env_offset: i32,
    thread_anchor_offset: i32,
    thread_state_offset: i32,
    osthread_id_offset: i32,
    anchor_sp_offset: i32,
    anchor_pc_offset: i32,
    frame_size_offset: i32,
    frame_complete_offset: i32,
    code_heap_addr: *const *const i8,
    code_heap_low_addr: *const *const i8,
    code_heap_high_addr: *const *const i8,
    code_heap_low: *const i8,
    code_heap_high: *const i8,
    code_heap_memory_offset: i32,
    code_heap_segmap_offset: i32,
    code_heap_segment_shift: i32,
    vs_low_bound_offset: i32,
    code_heap: [*const i8; 3],
    vs_high_bound_offset: i32,
    vs_low_offset: i32,
    vs_high_offset: i32,
    array_data_offset: i32,
    flag_name_offset: i32,
    flag_addr_offset: i32,
    flags_addr: *const i8,
    flag_count: i32,
    libjvm: Option<&'static CodeCache>,
    has_perm: bool,
    klass: jfieldID,
    tid: jfieldID,
    eetop: jfieldID,
    has_class_names: bool,
    has_method_structs: bool,
    has_native_thread_id: bool,
}

impl Display for VMStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _ = write!(f, "klass_name_offset:{:#X}", self.klass_name_offset);
        let _ = write!(f, ", symbol_length_offset:{:#X}", self.symbol_length_offset);
        let _ = write!(
            f,
            ", symbol_length_and_refcount_offset:{:#X}",
            self.symbol_length_and_refcount_offset
        );
        let _ = write!(f, ", symbol_body_offset:{:#X}", self.symbol_body_offset);
        let _ = write!(f, ", nmethod_name_offset:{:#X}", self.nmethod_name_offset);
        let _ = write!(
            f,
            ", nmethod_method_offset:{:#X}",
            self.nmethod_method_offset
        );
        let _ = write!(f, ", nmethod_entry_offset:{:#X}", self.nmethod_entry_offset);
        let _ = write!(f, ", nmethod_state_offset:{:#X}", self.nmethod_state_offset);
        let _ = write!(f, ", nmethod_level_offset:{:#X}", self.nmethod_level_offset);
        let _ = write!(
            f,
            ", method_constmethod_offset:{:#X}",
            self.method_constmethod_offset
        );
        let _ = write!(f, ", method_code_offset:{:#X}", self.method_code_offset);
        let _ = write!(
            f,
            ", constmethod_constants_offset:{:#X}",
            self.constmethod_constants_offset
        );
        let _ = write!(
            f,
            ", constmethod_idnum_offset:{:#X}",
            self.constmethod_idnum_offset
        );
        let _ = write!(f, ", pool_holder_offset:{:#X}", self.pool_holder_offset);
        let _ = write!(
            f,
            ", class_loader_data_offset:{:#X}",
            self.class_loader_data_offset
        );
        let _ = write!(f, ", methods_offset:{:#X}", self.methods_offset);
        let _ = write!(f, ", jmethod_ids_offset:{:#X}", self.jmethod_ids_offset);
        let _ = write!(
            f,
            ", class_loader_data_next_offset:{:#X}",
            self.class_loader_data_next_offset
        );
        let _ = write!(f, ", klass_offset_addr:{:#p}", self.klass_offset_addr);
        let _ = write!(
            f,
            ", thread_osthread_offset:{:#X}",
            self.thread_osthread_offset
        );
        let _ = write!(f, ", thread_anchor_offset:{:#X}", self.thread_anchor_offset);
        let _ = write!(f, ", thread_state_offset:{:#X}", self.thread_state_offset);
        let _ = write!(f, ", osthread_id_offset:{:#X}", self.osthread_id_offset);
        let _ = write!(f, ", anchor_sp_offset:{:#X}", self.anchor_sp_offset);
        let _ = write!(f, ", anchor_pc_offset:{:#X}", self.anchor_pc_offset);
        let _ = write!(f, ", frame_size_offset:{:#X}", self.frame_size_offset);
        let _ = write!(
            f,
            ", frame_complete_offset:{:#X}",
            self.frame_complete_offset
        );
        let _ = write!(f, ", code_heap_addr:{:#X}", self.code_heap_addr as isize);
        let _ = write!(
            f,
            ", code_heap_low_addr: {:#X}",
            self.code_heap_low_addr as isize
        );
        let _ = write!(
            f,
            ", code_heap_high_addr: {:#X}",
            self.code_heap_high_addr as isize
        );
        let _ = write!(
            f,
            ", code_heap_memory_offset:{:#X}",
            self.code_heap_memory_offset
        );
        let _ = write!(
            f,
            ", code_heap_segmap_offset:{:#X}",
            self.code_heap_segmap_offset
        );
        let _ = write!(
            f,
            ", code_heap_segment_shift:{:#X}",
            self.code_heap_segment_shift
        );
        let _ = write!(f, ", vs_low_bound_offset:{:#X}", self.vs_low_bound_offset);
        let _ = write!(f, ", vs_high_bound_offset:{:#X}", self.vs_high_bound_offset);
        let _ = write!(f, ", vs_low_offset:{:#X}", self.vs_low_offset);
        let _ = write!(f, ", vs_high_offset:{:#X}", self.vs_high_offset);
        let _ = write!(f, ", array_data_offset:{:#X}", self.array_data_offset);
        let _ = write!(f, ", flag_name_offset:{:#X}", self.flag_name_offset);
        let _ = write!(f, ", flag_addr_offset:{:#X}", self.flag_addr_offset);
        let _ = write!(f, ", flags_addr:{:#X}", self.flags_addr as isize);
        let _ = write!(f, ", flag_count:{:#X}", self.flag_count);
        write!(f, ", has_perm:{} ", self.has_perm)
    }
}

impl VMStruct {
    pub fn new() -> Self {
        Self {
            has_perm: false,
            flags_addr: ptr::null(),
            klass_name_offset: -1,
            symbol_length_offset: -1,
            symbol_length_and_refcount_offset: -1,
            symbol_body_offset: -1,
            nmethod_name_offset: -1,
            nmethod_method_offset: -1,
            nmethod_entry_offset: -1,
            nmethod_state_offset: -1,
            nmethod_level_offset: -1,
            method_constmethod_offset: -1,
            method_code_offset: -1,
            constmethod_constants_offset: -1,
            constmethod_idnum_offset: -1,
            pool_holder_offset: -1,
            class_loader_data_offset: -1,
            methods_offset: -1,
            jmethod_ids_offset: -1,
            class_loader_data_next_offset: -1,
            klass_offset_addr: ptr::null(),
            thread_osthread_offset: -1,
            thread_anchor_offset: -1,
            thread_state_offset: -1,
            osthread_id_offset: -1,
            anchor_sp_offset: -1,
            anchor_pc_offset: -1,
            frame_size_offset: -1,
            frame_complete_offset: -1,
            code_heap_addr: ptr::null(),
            code_heap_low_addr: ptr::null(),
            code_heap_high_addr: ptr::null(),
            code_heap_memory_offset: -1,
            code_heap_segmap_offset: -1,
            code_heap_segment_shift: -1,
            vs_low_bound_offset: -1,
            vs_high_bound_offset: -1,
            vs_low_offset: -1,
            vs_high_offset: -1,
            array_data_offset: -1,
            flag_name_offset: -1,
            flag_addr_offset: -1,
            flag_count: -1,
            libjvm: None,
            klass: ptr::null_mut(),
            tid: ptr::null_mut(),
            eetop: ptr::null_mut(),
            has_class_names: false,
            code_heap:[ptr::null(); 3],
            has_method_structs: false,
            code_heap_low: ptr::null(),
            code_heap_high: ptr::null(),
            has_native_thread_id: false,
            thread_env_offset: -1,
        }
    }

    #[inline(always)]
    pub fn eetop(&self) -> jfieldID {
        self.eetop
    }

    #[inline(always)]
    pub fn tid(&self) -> jfieldID {
        self.tid
    }

    #[inline(always)]
    pub fn thread_osthread_offset(&self) -> i32 {
        self.thread_osthread_offset
    }

    #[inline(always)]
    pub fn osthread_id_offset(&self) -> i32 {
        self.osthread_id_offset
    }

    #[inline(always)]
    pub fn code_heap(&self) -> CodeHeap<'_> {
        CodeHeap::new(self)
    }

    pub fn initial(&mut self, libjvm: Option<&'static CodeCache>) {
        self.libjvm = libjvm;
        unsafe {
            self.initial_offset();
        }
    }

    unsafe fn resovle_thread(&mut self, jni: &JNIEnv) {
        let mut thread: jthread = ptr::null_mut();
        match get_vm_mut()
            .jvmti()
            .get_current_thread(&mut thread as _) {
                Some(s) if s == 0  => {},
                _ => return,
        };
        let thr_class = jni.get_class_object(thread).unwrap();
        self.tid = match jni.get_field_id(thr_class, c_str!("tid"), c_str!("J")) {
            Some(f) => f,
            None => return,
        };
        self.eetop = match jni.get_field_id(thr_class, c_str!("eetop"), c_str!("J")) {
            Some(f) => f,
            None => return,
        };

        let vm_thread = VMThread::from_java_thread(jni, thread);
        if let Some(vm_thread) = vm_thread {
            self.thread_env_offset = (jni.inner() as *const i8).offset_from(vm_thread.inner()) as _;
            self.has_native_thread_id = self.thread_osthread_offset > 0 && self.osthread_id_offset > 0;
        }
        
    }


    /// get the offset of the symbols
    unsafe fn initial_offset(&mut self) {
        let entry = self.find_symbol(b"gHotSpotVMStructs");
        let stride = self.find_symbol(b"gHotSpotVMStructEntryArrayStride");
        let type_off = self.find_symbol(b"gHotSpotVMStructEntryTypeNameOffset");
        let field_off = self.find_symbol(b"gHotSpotVMStructEntryFieldNameOffset");
        let offset_off = self.find_symbol(b"gHotSpotVMStructEntryOffsetOffset");
        let addr_off = self.find_symbol(b"gHotSpotVMStructEntryAddressOffset");
        if entry.is_none() || stride.is_none() {
            return;
        }
        let mut entry = entry.unwrap();
        let stride = stride.unwrap();
        let type_off = type_off.unwrap();
        let field_off = field_off.unwrap();
        let offset_off = offset_off.unwrap();
        let addr_off = addr_off.unwrap();
        macro_rules! asign_offset {
            ($p : expr) => {
                $p = *((entry + offset_off) as *const i32)
            };
        }
        loop {
            let typ = *((entry + type_off) as *const *const i8);
            let filed = *((entry + field_off) as *const *const i8);
            if typ.is_null() || filed.is_null() {
                break;
            }
            let typ_sl = CStr::from_ptr(typ).to_bytes();
            let filed_sl = CStr::from_ptr(filed).to_bytes();

            match typ_sl {
                b"Klass" => {
                    if filed_sl == b"_name" {
                        asign_offset!(self.klass_name_offset);
                    }
                }
                b"Symbol" => match filed_sl {
                    b"_length" => asign_offset!(self.symbol_length_offset),
                    b"_length_and_refcount" => {
                        asign_offset!(self.symbol_length_and_refcount_offset)
                    }
                    b"_body" => asign_offset!(self.symbol_body_offset),
                    _ => {}
                },
                b"CompiledMethod" | b"nmethod" => match filed_sl {
                    b"_method" => asign_offset!(self.nmethod_method_offset),
                    b"_verified_entry_point" => asign_offset!(self.nmethod_entry_offset),
                    b"_state" => asign_offset!(self.nmethod_state_offset),
                    b"_comp_level" => asign_offset!(self.nmethod_level_offset),
                    _ => {}
                },
                b"Method" => match filed_sl {
                    b"_constMethod" => asign_offset!(self.method_constmethod_offset),
                    b"_code" => asign_offset!(self.method_code_offset),
                    _ => {}
                },
                b"ConstMethod" => match filed_sl {
                    b"_constants" => asign_offset!(self.constmethod_constants_offset),
                    b"_method_idnum" => asign_offset!(self.constmethod_idnum_offset),
                    _ => {}
                },
                b"ConstantPool" => {
                    if filed_sl == b"_pool_holder" {
                        asign_offset!(self.pool_holder_offset);
                    }
                }
                b"InstanceKlass" => match filed_sl {
                    b"_class_loader_data" => asign_offset!(self.class_loader_data_offset),
                    b"_methods" => asign_offset!(self.methods_offset),
                    b"_methods_jmethod_ids" => asign_offset!(self.jmethod_ids_offset),
                    _ => {}
                },
                b"ClassLoaderData" => {
                    if filed_sl == b"_next" {
                        asign_offset!(self.class_loader_data_next_offset);
                    }
                }
                b"java_lang_Class" => {
                    if filed_sl == b"_klass_offset" {
                        self.klass_offset_addr = *((entry + addr_off) as *const *const i32)
                    }
                }
                b"JavaThread" => match filed_sl {
                    b"_osthread" => asign_offset!(self.thread_osthread_offset),
                    b"_anchor" => asign_offset!(self.thread_anchor_offset),
                    b"_thread_state" => asign_offset!(self.thread_state_offset),
                    _ => {}
                },
                b"OSThread" => {
                    if filed_sl == b"_thread_id" {
                        asign_offset!(self.osthread_id_offset);
                    }
                }
                b"JavaFrameAnchor" => match filed_sl {
                    b"_last_Java_sp" => asign_offset!(self.anchor_sp_offset),
                    b"_last_Java_pc" => asign_offset!(self.anchor_pc_offset),
                    _ => {}
                },
                b"CodeBlob" => match filed_sl {
                    b"_frame_size" => asign_offset!(self.frame_size_offset),
                    b"_frame_complete_offset" => asign_offset!(self.frame_complete_offset),
                    b"_name" => asign_offset!(self.nmethod_name_offset),
                    _ => {}
                },
                b"CodeCache" => match filed_sl {
                    b"_heap" | b"_heaps" => {
                        self.code_heap_addr = *((entry + addr_off) as *const *const *const i8)
                    }
                    b"_low_bound" => {
                        self.code_heap_low_addr = *((entry + addr_off) as *const *const *const i8)
                    }
                    b"_high_bound" => {
                        self.code_heap_high_addr = *((entry + addr_off) as *const *const *const i8)
                    }
                    _ => {}
                },
                b"CodeHeap" => match filed_sl {
                    b"_memory" => asign_offset!(self.code_heap_memory_offset),
                    b"_segmap" => asign_offset!(self.code_heap_segmap_offset),
                    b"_log2_segment_size" => asign_offset!(self.code_heap_segment_shift),
                    _ => {}
                },
                b"VirtualSpace" => match filed_sl {
                    b"_low_boundary" => asign_offset!(self.vs_low_bound_offset),
                    b"_high_boundary" => asign_offset!(self.vs_high_bound_offset),
                    b"_low" => asign_offset!(self.vs_low_offset),
                    b"_high" => asign_offset!(self.vs_high_offset),
                    _ => {}
                },
                b"GrowableArray<int>" => {
                    if filed_sl == b"_data" {
                        asign_offset!(self.array_data_offset);
                    }
                }
                b"JVMFlag" | b"Flag" => match filed_sl {
                    b"_name" | b"name" => asign_offset!(self.flag_name_offset),
                    b"_addr" | b"addr" => asign_offset!(self.flag_addr_offset),
                    b"flags" => self.flags_addr = **((entry + addr_off) as *const *const *const i8),
                    b"numFlags" => self.flag_count = **((entry + addr_off) as *const *const i32),
                    _ => {}
                },
                b"PermGen" => {
                    self.has_perm = true;
                }
                _ => {}
            }
            entry += stride;
        }
    }

    #[inline(always)]
    unsafe fn find_symbol(&self, name: &[u8]) -> Option<usize> {
        self.libjvm
            .and_then(|libjvm| libjvm.find_symbol(name))
            .map(|p| *(p as *const usize))
    }

    pub fn ready(&mut self) {
        unsafe {
            self.resovle_offset();
            get_vm_mut().get_jni_env().map(|jni| {
                self.resovle_thread(&jni);
            });
        }
    } 

    pub unsafe fn resovle_offset(&mut self) {
        if !self.klass_offset_addr.is_null() {
            self.klass = ((*self.klass_offset_addr)<<2|2) as uintptr_t as jfieldID
        }
        self.has_class_names = self.klass_name_offset >= 0
            && (self.symbol_length_offset >= 0 || self.symbol_length_and_refcount_offset >= 0)
            && self.symbol_body_offset >= 0
            && !self.klass.is_null();
        self.has_method_structs =self.jmethod_ids_offset >= 0
            && self.nmethod_method_offset >= 0
            && self.nmethod_entry_offset >= 0
            && self.nmethod_state_offset >= 0
            && self.method_constmethod_offset >= 0
            && self.method_code_offset >= 0
            && self.constmethod_constants_offset >= 0
            && self.constmethod_idnum_offset >= 0
            && self.pool_holder_offset >= 0;
        // hotspot the heap
        if !self.code_heap_addr.is_null() && 
            !self.code_heap_low_addr.is_null() &&
            !self.code_heap_high_addr.is_null() {
            let code_heaps = *self.code_heap_addr;
            let code_heap_count = *(code_heaps as *const i32);
            if code_heap_count <= 3 && self.array_data_offset >= 0 {
                let code_heap_array = *(code_heaps.add(self.array_data_offset as _) as *const *const i8);
                for i in 0..code_heap_count as _ {
                    self.code_heap[i] = *((code_heap_array as *const *const i8).add(i));
                }
            }
            self.code_heap_low = *self.code_heap_low_addr;
            self.code_heap_high = *self.code_heap_high_addr;
        }
        if !self.code_heap[0].is_null() && self.code_heap_segment_shift >= 0 {
            // aquire the segment shift from heap
            self.code_heap_segment_shift = *(self.code_heap[0].add(self.code_heap_segment_shift as _) as *const i32);
        }
        if self.code_heap_memory_offset < 0 || self.code_heap_segmap_offset < 0 ||
            self.code_heap_segment_shift < 0 || self.code_heap_segment_shift > 16 {
            self.code_heap = [ptr::null(); 3];
        }
    }

    #[inline(always)]
    pub fn has_method_structs(&self) ->  bool {
        self.has_method_structs
    }

    #[inline(always)]
    pub fn has_native_thread_id(&self) ->  bool {
        self.has_native_thread_id
    }

    #[inline(always)]
    pub fn thread_env_offset(&self) ->  i32 {
        self.thread_env_offset
    }
}
