use common::Syscall;
use core::arch::asm;

pub fn put_char(ch: u8) {
    unsafe { syscall(ch as u64, 0, 0, Syscall::PUTCHAR) };
}

pub fn get_char() -> u8 {
    (unsafe { syscall(0, 0, 0, Syscall::GETCHAR) }) as u8
}

pub fn exit() -> ! {
    unsafe {
        syscall(0, 0, 0, Syscall::EXIT);
    }
    unreachable!()
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
