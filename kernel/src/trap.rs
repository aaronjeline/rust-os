use crate::{println, read_csr};
use core::arch::naked_asm;

#[macro_export]
macro_rules! read_csr {
    ($csr:expr) => {{
        let mut value: u64;
        unsafe {
            ::core::arch::asm!(concat!("csrr {}, ", $csr), out(reg) value);
        }
        value
    }};
}

#[macro_export]
macro_rules! write_csr {
    ($csr:expr, $v:expr) => {{
        unsafe {
            ::core::arch::asm!(concat!("csrw ", $csr, ", {}"), in(reg) $v);
        }
    }};
}

#[repr(packed)]
struct TrapFrame {
    x1: u64,
    x2: u64,
    x3: u64,
    x4: u64,
    x5: u64,
    x6: u64,
    x7: u64,
    x8: u64,
    x9: u64,
    x10: u64,
    x11: u64,
    x12: u64,
    x13: u64,
    x14: u64,
    x15: u64,
    x16: u64,
    x17: u64,
    x18: u64,
    x19: u64,
    x20: u64,
    x21: u64,
    x22: u64,
    x23: u64,
    x24: u64,
    x25: u64,
    x26: u64,
    x27: u64,
    x28: u64,
    x29: u64,
    x30: u64,
    x31: u64,
    sp: u64,
}

#[unsafe(no_mangle)]
#[unsafe(naked)]
#[unsafe(link_section = ".text.stvec")]
pub unsafe extern "C" fn trap_vector() {
    naked_asm!(
        "csrrw sp, sscratch, sp", // Retrieve the kernel stack of the running process
        "addi sp, sp, -256",      // Allocate 8 * 32 registers of space
        "sd x1, 0(sp)",
        "sd x2, 8(sp)",
        "sd x3, 16(sp)",
        "sd x4, 24(sp)",
        "sd x5, 32(sp)",
        "sd x6, 40(sp)",
        "sd x7, 48(sp)",
        "sd x8, 56(sp)",
        "sd x9, 64(sp)",
        "sd x10, 72(sp)",
        "sd x11, 80(sp)",
        "sd x12, 88(sp)",
        "sd x13, 96(sp)",
        "sd x14, 104(sp)",
        "sd x15, 112(sp)",
        "sd x16, 120(sp)",
        "sd x17, 128(sp)",
        "sd x18, 136(sp)",
        "sd x19, 144(sp)",
        "sd x20, 152(sp)",
        "sd x21, 160(sp)",
        "sd x22, 168(sp)",
        "sd x23, 176(sp)",
        "sd x24, 184(sp)",
        "sd x25, 192(sp)",
        "sd x26, 200(sp)",
        "sd x27, 208(sp)",
        "sd x28, 216(sp)",
        "sd x29, 224(sp)",
        "sd x30, 232(sp)",
        "sd x31, 240(sp)",
        "csrr a0, sscratch", // Retrive and save sp at time of exception
        "sw a0, 248(sp)",
        "addi a0, sp, 8 * 31",
        "csrw sscratch, a0",
        "mv a0, sp", // Restore the stack before calling handler
        "call trap_handler",
        "ld x1, 0(sp)",
        "ld x2, 8(sp)",
        "ld x3, 16(sp)",
        "ld x4, 24(sp)",
        "ld x5, 32(sp)",
        "ld x6, 40(sp)",
        "ld x7, 48(sp)",
        "ld x8, 56(sp)",
        "ld x9, 64(sp)",
        "ld x10, 72(sp)",
        "ld x11, 80(sp)",
        "ld x12, 88(sp)",
        "ld x13, 96(sp)",
        "ld x14, 104(sp)",
        "ld x15, 112(sp)",
        "ld x16, 120(sp)",
        "ld x17, 128(sp)",
        "ld x18, 136(sp)",
        "ld x19, 144(sp)",
        "ld x20, 152(sp)",
        "ld x21, 160(sp)",
        "ld x22, 168(sp)",
        "ld x23, 176(sp)",
        "ld x24, 184(sp)",
        "ld x25, 192(sp)",
        "ld x26, 200(sp)",
        "ld x27, 208(sp)",
        "ld x28, 216(sp)",
        "ld x29, 224(sp)",
        "ld x30, 232(sp)",
        "ld x31, 240(sp)",
        // Load stack pointer
        "ld sp, 248(sp)",
        // Return
        "sret"
    );
}

// #[unsafe(link_section = ".text.stvec")]
#[unsafe(no_mangle)]
extern "C" fn trap_handler(_frame: *const TrapFrame) -> ! {
    println!("In trap handler");
    let scause = read_csr!("scause");
    let sepc = read_csr!("sepc");
    let stval = read_csr!("stval");
    let str = match scause {
        0 => "instr address misalign",
        1 => "instruction access fault",
        2 => "illegal instruction",
        3 => "breakpoint",
        4 => "load address misaligned",
        5 => "load access fault",
        6 => "store/AMO address misaligned",
        7 => "store/AMO access fault",
        8 => "environment call from U/VU-mode",
        9 => "environment call from HS-mode",
        10 => "environment call from VS-mode",
        11 => "environment call from M-mode",
        12 => "instruction page fault",
        13 => "load page fault",
        15 => "store/AMO page fault",
        20 => "instruction guest-page fault",
        21 => "load guest-page fault",
        22 => "virtual instruction",
        23 => "store/AMO guest-page fault",
        0x8000_0000_0000_0000 => "user software interrupt",
        0x8000_0000_0000_0001 => "supervisor software interrupt",
        0x8000_0000_0000_0002 => "hypervisor software interrupt",
        0x8000_0000_0000_0003 => "machine software interrupt",
        0x8000_0000_0000_0004 => "user timer interrupt",
        0x8000_0000_0000_0005 => "supervisor timer interrupt",
        0x8000_0000_0000_0006 => "hypervisor timer interrupt",
        0x8000_0000_0000_0007 => "machine timer interrupt",
        0x8000_0000_0000_0008 => "user external interrupt",
        0x8000_0000_0000_0009 => "supervisor external interrupt",
        0x8000_0000_0000_000a => "hypervisor external interrupt",
        0x8000_0000_0000_000b => "machine external interrupt",
        _ => panic!("unknown scause: {:#x}", scause),
    };

    panic!("trap handler: {} at {:#x} (stval={:#x})", str, sepc, stval);
}
