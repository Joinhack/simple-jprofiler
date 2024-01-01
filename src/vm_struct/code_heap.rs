use std::{mem, sync::atomic::Ordering};

use super::{nmethod::NMethod, VMStruct};

pub struct CodeHeap<'a>(&'a VMStruct);

impl<'a> CodeHeap<'a> {
    #[inline(always)]
    pub fn new(vmstruct: &'a VMStruct) -> Self {
        Self(vmstruct)
    }

    pub unsafe fn code_contains(&self, pc: *const i8) -> bool {
        self.0.code_heap_low.load(Ordering::Acquire) <= pc as _
            && pc < self.0.code_heap_high.load(Ordering::Acquire)
    }

    unsafe fn contain(&self, heap: *const i8, pc: *const i8) -> bool {
        if heap.is_null() {
            return false;
        }
        let mem_low_off = (self.0.code_heap_memory_offset + self.0.vs_low_offset) as _;
        let mem_hight_off = (self.0.code_heap_memory_offset + self.0.vs_high_offset) as _;
        pc >= *(heap.add(mem_low_off) as *const *const i8)
            && pc < *(heap.add(mem_hight_off) as *const *const i8)
    }

    unsafe fn find_nmethod_in_heap(&self, heap: *const i8, pc: *const i8) -> Option<NMethod> {
        let start_off = (self.0.code_heap_memory_offset + self.0.vs_low_offset) as _;
        let heap_start = *(heap.add(start_off) as *const *const i8);
        let segmap_off = (self.0.code_heap_segmap_offset + self.0.vs_low_offset) as _;
        let segmap = *(heap.add(segmap_off) as *const *const i8);
        let mut idx = (pc.sub(heap_start as _)) as isize >> self.0.code_heap_segment_shift as isize;
        let mut seg = *segmap.offset(idx);
        if seg as u8 == 0xff {
            return None;
        }
        while seg > 0 {
            idx -= seg as isize;
            seg = *segmap.offset(idx);
        }
        let block = heap_start.add((idx << self.0.code_heap_segment_shift) as usize);
        if (*block.add(mem::size_of::<isize>())) > 0 {
            let nmethod_addr = block.add(2 * mem::size_of::<isize>());
            Some(NMethod::new(nmethod_addr, self.0.nmethod_name_offset))
        } else {
            None
        }
    }

    pub unsafe fn find_nmethod(&self, pc: *const i8) -> Option<NMethod> {
        if self.contain(self.0.code_heap[0], pc) {
            return self.find_nmethod_in_heap(self.0.code_heap[0], pc);
        }
        if self.contain(self.0.code_heap[1], pc) {
            return self.find_nmethod_in_heap(self.0.code_heap[1], pc);
        }
        if self.contain(self.0.code_heap[2], pc) {
            return self.find_nmethod_in_heap(self.0.code_heap[2], pc);
        }
        return None;
    }
}
