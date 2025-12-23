use core::arch::naked_asm;

unsafe extern "C" {
    pub static mut __stack_top: u8;
}

pub extern "C" fn exit() -> ! {
    loop {}
}

pub fn putchar(c: char) {
    todo!()
}
