use crate::{println, process::do_yield};

#[derive(Debug, Clone, Copy)]
pub struct SbiReturn {
    pub error: u64,
    pub value: u64,
}

pub unsafe fn sbi_call(
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    fid: u64,
    eid: u64,
) -> SbiReturn {
    let error: u64;
    let value: u64;

    unsafe {
        core::arch::asm!(
            "ecall",
            inout("a0") arg0 => error,
            inout("a1") arg1 => value,
            in("a2") arg2,
            in("a3") arg3,
            in("a4") arg4,
            in("a5") arg5,
            in("a6") fid,
            in("a7") eid,
            options(nostack)
        );
    }

    SbiReturn { error, value }
}

pub fn putchar(ch: u8) {
    unsafe {
        sbi_call(ch as u64, 0, 0, 0, 0, 0, 0, 1);
    }
}

pub fn getchar() -> Option<u8> {
    let value = unsafe { sbi_call(0, 0, 0, 0, 0, 0, 0, 2).error } as i64;
    if value >= 0 { Some(value as u8) } else { None }
}

pub fn getchar_coop() -> u8 {
    loop {
        match getchar() {
            None => do_yield(),
            Some(chr) => return chr,
        }
    }
}
