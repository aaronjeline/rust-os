unsafe extern "C" {
    pub static mut __kernel_start: u8;
    pub static mut __bss: u8;
    pub static mut __bss_end: u8;
    pub static mut __heap: u8;
    pub static mut __heap_end: u8;
}

pub const USER_BASE: usize = 0x1000000;

//pub const SHELL: &[u8] = include_bytes!("../../shell.bin");
pub const SHELL: &[u8] = &[];
