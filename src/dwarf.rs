const DW_STACK_SLOT: i32 = std::mem::size_of::<*const()>() as _;

#[cfg(target_pointer_width= "64")]
mod target64 {
    pub const DWARF_SUPPORTED: bool = true;
    pub const DW_REG_FP: i32 = 6;
    pub const DW_REG_SP: i32 = 7;
    pub const DW_REG_PC: i32 = 8;
}

#[cfg(target_pointer_width= "64")]
use target64::*;


#[cfg(target_pointer_width= "32")]
mod target32 {
    pub const DWARF_SUPPORTED: bool = true;
    pub const DW_REG_FP: i32 = 5;
    pub const DW_REG_SP: i32 = 4;
    pub const DW_REG_PC: i32 = 8;
}

#[cfg(target_pointer_width= "32")]
use target32::*;


static DEFAULT_FRAME: FrameDesc = FrameDesc {loc: 0,cfa: DW_REG_FP | (2 * DW_STACK_SLOT) << 8,fp_off:0 };

pub struct FrameDesc {
    pub loc: u32,
    pub cfa: i32,
    pub fp_off: i32,
}