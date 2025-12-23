use core::arch::asm;

use crate::println;
pub fn putchar(ch: u8) {
    unsafe { syscall(ch as u64, 0, 0, Syscall::PUTCHAR) };
}

pub fn getchar() -> u8 {
    (unsafe { syscall(0, 0, 0, Syscall::GETCHAR) }) as u8
}

#[derive(Debug, Clone, Copy)]
enum Syscall {
    PUTCHAR,
    GETCHAR,
}

impl Into<u64> for Syscall {
    fn into(self) -> u64 {
        match self {
            Self::PUTCHAR => 3,
            Self::GETCHAR => 2,
        }
    }
}

unsafe fn syscall(arg0: u64, arg1: u64, arg2: u64, sysno: Syscall) -> u64 {
    let result: u64;
    let sysno: u64 = sysno.into();
    unsafe {
        asm!(
            "ecall",
            inout("a0") arg0 => result,
            in("a1") arg1,
            in("a2") arg2,
            in("a3") sysno,
        );
    }
    result
}
