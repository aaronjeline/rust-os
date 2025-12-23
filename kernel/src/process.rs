use crate::{
    constants::{self, USER_BASE},
    memory::{self, PAGE_SIZE, PTE, Paddr, PageFlags, Vaddr, alloc_pages},
    println, write_csr,
};
use core::{
    arch::{asm, naked_asm},
    mem::transmute,
    ptr,
};

/// Process stack size
const PROC_STACK_SIZE: usize = 8192;
/// Size of our process table
const PROCS_MAX: usize = 8;

/// A process in the system
#[derive(Debug)]
pub struct Process {
    pid: Pid,
    sp: u64,
    page_table: *mut PTE,
    stack: [u8; PROC_STACK_SIZE],
}

impl Process {
    /// Produce an uninitialized process structure
    /// _none_ of the fields in here are valid after this call
    /// Especially `page_table`
    /// It is the responsibility of the caller to initialize these fields
    pub unsafe fn uninitialized() -> Self {
        Self {
            pid: Pid::idle(),
            sp: 0,
            page_table: 0 as *mut PTE,
            stack: [0; PROC_STACK_SIZE],
        }
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// Get a mutable raw pointer to the the stack of this process
    pub fn get_mut_sp(&mut self) -> *mut u64 {
        (&mut self.sp) as *mut u64
    }

    /// Is this process the idle process?
    pub fn is_idle_process(&self) -> bool {
        self.pid.is_idle()
    }
}

impl core::fmt::Display for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Process {} - current sp {:#x}", self.pid, self.sp)
    }
}

/// The process scheduler
pub struct Scheduler {
    /// The active process table
    procs: [Option<Process>; PROCS_MAX],
    /// The pid of the currently running process
    current: Pid,
}

impl Scheduler {
    /// Creates a new schedular, with an idle process
    pub const fn new() -> Self {
        let idle = Process {
            pid: Pid::idle(),
            sp: 0,
            page_table: 0 as *mut PTE,
            stack: [0; PROC_STACK_SIZE],
        };
        Self {
            procs: [Some(idle), None, None, None, None, None, None, None],
            current: Pid::idle(),
        }
    }

    /// Get a reference to the next process to switch to
    fn find_next_process(&self) -> &Process {
        for i in 1..=PROCS_MAX {
            let maybe_proc = self.procs[(self.current.as_usize() + i) % PROCS_MAX].as_ref();
            match maybe_proc {
                Some(proc) if proc.is_idle_process() => continue,
                Some(proc) => return proc,
                None => continue,
            }
        }
        unreachable!()
    }

    /// Cooperative yield.
    /// Switch between running process
    fn do_yield(&mut self) {
        let next = self.find_next_process();
        // If we decide to switch to the same process, we're done
        if self.current != next.pid() {
            // We've gotta switch running processes
            // Step 1:
            // Switch to the new process's page table
            // Create the constant that goes in the SATP CSR
            // Composed of:
            // The address of the page table divided by the page size
            // A flag that tells the processor we're using SV39 paging
            let satp: usize = (8 << 60) | (next.page_table as usize / PAGE_SIZE);
            // Write the value into the register, using memory fences
            unsafe { asm!("sfence.vma", "csrw satp, {satp}", "sfence.vma", satp = in(reg) satp) };

            // Step 2: Save a trusted pointer to the kernel stack
            // Store the pointer to the bottom of the next stack in sscratch
            unsafe {
                let stack = next.stack.as_ptr().add(PROC_STACK_SIZE);
                write_csr!("sscratch", stack as u64);
            }

            // Step 3: Swap the active process in the scheduler
            let prev = self.current;
            self.current = next.pid();
            // Step 4: execute the context switch
            unsafe {
                switch_context(
                    self.get_mut(prev).unwrap().get_mut_sp(),
                    self.get_mut(self.current).unwrap().get_mut_sp(),
                )
            };
        }
    }

    /// Get an iterator over the currently running processes
    pub fn running_processes(&self) -> impl Iterator<Item = &Process> {
        self.procs.iter().filter_map(|x| x.as_ref())
    }

    /// Get a shared reference to an entry in the process table
    pub fn get(&self, pid: Pid) -> Option<&Process> {
        self.procs[pid.as_usize()].as_ref()
    }

    /// Get a mutable reference to an entry in the process table
    pub fn get_mut(&mut self, pid: Pid) -> Option<&mut Process> {
        self.procs[pid.as_usize()].as_mut()
    }

    /// Finds the next free process in the process table and initializes it's PID
    /// _all other_ fields are uninitialized!
    /// PANICS: if there are no new process slots.
    unsafe fn find_free_process(&mut self) -> &mut Process {
        for (i, opt_proc) in self.procs.iter_mut().enumerate() {
            if opt_proc.is_none() {
                *opt_proc = Some(unsafe { Process::uninitialized() });
                let proc = opt_proc.as_mut().unwrap();
                (*proc).pid = Pid::new(i);
                return proc;
            }
        }
        panic!("No free processes");
    }

    /// Creates a new process that will execute the code at `pc`
    pub fn create_process(&mut self, image: &[u8]) -> Pid {
        // We are about to initialize proc
        let proc = unsafe { self.find_free_process() };
        // Allocate a page that will hold the process's page table
        (*proc).page_table = alloc_pages(1) as *mut PTE;

        // Map kernel memory

        let start = &raw mut constants::__kernel_start as usize;
        let end = &raw mut constants::__heap_end as usize;

        for addr in (start..end).step_by(PAGE_SIZE) {
            // while addr < &raw mut constants::__kernel_start {
            let vaddr = Vaddr(addr as u64);
            // The kernel lives in low memory, and each page points to the numerically same frame
            memory::map_page(
                proc.page_table,
                vaddr,
                Paddr(addr as *mut u8),
                PageFlags::kernel_all(),
            );
        }

        // Map user pages
        let image_size = image.len();

        for offset in (0..image_size).step_by(PAGE_SIZE) {
            let page = alloc_pages(1);
            let remaining = image_size - offset;
            let copy_size = PAGE_SIZE.min(remaining);
            unsafe {
                ptr::copy_nonoverlapping(image[offset..].as_ptr(), page, copy_size);
            }
            memory::map_page(
                proc.page_table,
                Vaddr((USER_BASE + offset) as u64),
                Paddr(page),
                PageFlags::all(),
            );
        }

        // Initialize the sp to look like switch_context had saved registers
        let base = proc.stack.as_mut_ptr();
        let stack_top = unsafe { base.add(PROC_STACK_SIZE) };

        unsafe {
            // Allocate space for 14 saved registers (ra + s0-s11 + one extra for alignment)
            const NUM_REGISTERS: usize = 14;
            let sp: *mut u64 = transmute(stack_top);
            let sp = sp.sub(NUM_REGISTERS);

            // Set up the saved register area
            *sp.add(0) = user_entry as u64; // ra = entry point
            // s0-s11 are initialized to 0 (stack is already zeroed)

            // Store the sp pointing to the saved register area
            proc.sp = sp as u64;
        }
        return proc.pid;
    }
}

/// Executes a context switch,
/// saving callee save registers on the stack
#[unsafe(naked)]
unsafe extern "C" fn switch_context(prev_sp: *const u64, next_sp: *const u64) {
    const NUM_REGS: usize = 14;
    naked_asm!(
        "addi sp, sp, -{num_regs} * {size}", // Allocate space on stack
        // Save callee-save registers
        "sd ra, 0 * {size}(sp)",
        "sd s0, 1 * {size}(sp)",
        "sd s1, 2 * {size}(sp)",
        "sd s2, 3 * {size}(sp)",
        "sd s3, 4 * {size}(sp)",
        "sd s4, 5 * {size}(sp)",
        "sd s5, 6 * {size}(sp)",
        "sd s6, 7 * {size}(sp)",
        "sd s7, 8 * {size}(sp)",
        "sd s8, 9 * {size}(sp)",
        "sd s9, 10 * {size}(sp)",
        "sd s10, 11 * {size}(sp)",
        "sd s11, 12 * {size}(sp)",
        // Switch stack pointer
        "sd sp, (a0)\n", // *prev_sp = sp
        "ld sp, (a1)", // sp = *next_sp
        // Restore
        "ld ra, 0 * {size}(sp)",
        "ld s0, 1 * {size}(sp)",
        "ld s1, 2 * {size}(sp)",
        "ld s2, 3 * {size}(sp)",
        "ld s3, 4 * {size}(sp)",
        "ld s4, 5 * {size}(sp)",
        "ld s5, 6 * {size}(sp)",
        "ld s6, 7 * {size}(sp)",
        "ld s7, 8 * {size}(sp)",
        "ld s8, 9 * {size}(sp)",
        "ld s9, 10 * {size}(sp)",
        "ld s10, 11 * {size}(sp)",
        "ld s11, 12 * {size}(sp)",
        // Restore stack
        "addi sp, sp, {num_regs} * {size}",
        "ret",
        "ebreak",
        num_regs = const NUM_REGS,
        size = const 8);
}

impl core::fmt::Display for Scheduler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "===Schedular===")?;
        writeln!(f, "Current active process: {}", self.current)?;
        writeln!(f, "Process Table:")?;
        for proc in self.procs.iter() {
            match proc {
                None => writeln!(f, "<unallocated process>")?,
                Some(p) => writeln!(f, "{p}")?,
            }
        }
        Ok(())
    }
}
/// Wrapper type for process-ids
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pid(usize);

impl core::fmt::Display for Pid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Pid {
    /// Create a new Pid at a given index
    pub fn new(idx: usize) -> Self {
        assert!(idx != 0, "Tried to create an idle process");
        Self(idx)
    }

    /// Get the pid for the idle process
    pub const fn idle() -> Self {
        Self(0)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }

    pub fn is_idle(self) -> bool {
        self.0 == 0
    }
}

static mut GLOBAL_SCHEDULER: Scheduler = Scheduler::new();

/// Global function to execute a cooperative task switch
pub fn do_yield() {
    unsafe {
        (*core::ptr::addr_of_mut!(GLOBAL_SCHEDULER)).do_yield();
    }
}

/// Global function to create a new process
pub fn create_process(image: &[u8]) -> Pid {
    unsafe { (*core::ptr::addr_of_mut!(GLOBAL_SCHEDULER)).create_process(image) }
}

/// Global function to list the current process table
pub fn ps() {
    unsafe {
        let ptr = core::ptr::addr_of!(GLOBAL_SCHEDULER);
        println!("{}", *ptr);
    }
}

const STATUS_PIE: u64 = 1 << 5;

#[unsafe(naked)]
extern "C" fn user_entry() {
    naked_asm!(
        "li t0, {sepc}",
        "csrw sepc, t0",
        "li t1, {sstatus}",
        "csrw sstatus, t1",
        "sret",
        sepc = const USER_BASE,
        sstatus = const STATUS_PIE,
    )
}
