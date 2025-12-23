#![no_std]
#![no_main]

use core::{arch::naked_asm, panic::PanicInfo};
#[macro_use]
mod user;
mod print;
mod syscall;
use user::exit;

use crate::syscall::{getchar, putchar};

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
    loop {
        print!("> ");
        // let mut buf = Vec::with_capacity(512);
        let mut buf = [0; 512];
        for i in 0..buf.len() {
            let c = getchar();

            putchar(c);
            if c == b'\r' {
                print!("\n");
                break;
            } else {
                buf[i] = c;
            }
        }
        if &buf[..5] == b"hello" {
            println!("hello world!");
        } else {
            println!("Unknown command",);
        }
    }
}
