#![no_std]
#![no_main]

extern crate alloc;
mod allocator;
mod constants;
mod dummy_procs;
mod memory;
mod process;
mod sbi;
mod virtio;
#[macro_use]
mod print;
#[macro_use]
mod trap;

use alloc::slice;
use alloc::string::String;
use constants::*;
use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::copy_nonoverlapping;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn boot() -> ! {
    unsafe {
        asm!(
            "la sp, __stack_top", // Load __stack_top address into sp
            "j {main}",           // Jump to main
            main = sym main,
            options(noreturn) // No return
        )
    }
}

#[panic_handler]
pub fn panic_handler(info: &PanicInfo) -> ! {
    println!("Panic: {info}");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

fn main() -> ! {
    unsafe {
        let bss_start = &raw mut __bss;
        let bss_size = (&raw mut __bss_end as usize) - (&raw mut __bss as usize);
        core::ptr::write_bytes(bss_start, 0, bss_size);
        // asm!("csrw stvec, {}", in(reg) trap::trap_vector as usize);
        write_csr!("stvec", trap::trap_vector as usize);
        println!(
            "Trap handler initialized at {:#x}",
            trap::trap_vector as usize
        );
        let stvec_val = read_csr!("stvec");
        println!("stvec set to: {:#x}", stvec_val);
    }

    allocator::GLOBAL_ALLOCATOR.init(&raw mut __heap, &raw mut __heap_end);
    println!("Allocator initialized!");

    let mut driver = virtio::BlockDeviceDriver::new();
    let mut buf = [0; 512];
    driver.disk_read(&mut buf, 0).unwrap();
    let s = String::from_utf8_lossy(&buf);
    println!("First sector {s}");
    let source = b"Hello from kernel!\n";
    unsafe {
        copy_nonoverlapping(source.as_ptr(), buf.as_mut_ptr(), source.len());
    }

    driver.disk_write(&buf, 0).unwrap();

    process::create_process(constants::SHELL);

    process::ps();

    process::do_yield();

    loop {}
}
