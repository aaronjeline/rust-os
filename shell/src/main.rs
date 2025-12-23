#![no_std]
#![no_main]

use core::{arch::naked_asm, panic::PanicInfo};
mod user;
use user::exit;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = ".text.start")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn start() {
    naked_asm!(
        "la sp, __stack_top",
        "call {main}",
        "call {exit}",
        main = sym main,
        exit = sym exit,
    )
}

fn main() {
    loop {}
}
