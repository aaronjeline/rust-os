use crate::println;
use crate::{process, sbi::putchar};
use core::arch::asm;

pub fn delay() {
    for _ in 0..300000 {
        unsafe {
            asm!("nop");
        }
    }
}

pub extern "C" fn proc_a_entry() {
    println!("Starting a");
    loop {
        putchar(b'A');
        process::do_yield();
        delay();
    }
}

pub extern "C" fn proc_b_entry() {
    println!("Starting b");
    loop {
        putchar(b'b');
        process::do_yield();
        delay();
    }
}
