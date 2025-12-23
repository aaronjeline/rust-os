#![no_std]
#[macro_use]
pub mod print;
pub mod syscall;
unsafe extern "C" {
    pub static mut __stack_top: u8;
}
