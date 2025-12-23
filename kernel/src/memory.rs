use core::alloc::{GlobalAlloc, Layout};

use crate::allocator;

pub const PAGE_SIZE: usize = 4096;

#[derive(Copy, Clone)]
pub struct Paddr(pub *mut u8);

impl core::fmt::Debug for Paddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(Phys: 0x{:#x})", self.0 as u64)
    }
}

/// A virtual address in the RISC V 64 bit Sv39 paging system.
/// 63   39 38           30 29           21 20           12 11            0
/// +-------+---------------+---------------+---------------+---------------+
/// | Unused|   L2 Index    |   L1 Index    |   L0 Index    |    Offset     |
/// | (must |   (9 bits)    |   (9 bits)    |   (9 bits)    |   (12 bits)   |
/// | be 0) |               |               |               |               |
/// +-------+---------------+---------------+---------------+---------------+

#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Vaddr(pub u64);

impl Vaddr {
    pub fn as_number(self) -> u64 {
        self.0
    }

    pub fn is_aligned(self) -> bool {
        (self.0 & 0xFFF) == 0
    }

    pub fn pt_index_for_level(self, level: usize) -> usize {
        ((self.0 >> (12 + level * 9)) & 0x1FF) as usize
    }
}

impl core::fmt::Debug for Vaddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "(Virt: 0x{:#x})", self.as_number())
    }
}

impl core::ops::Add for Vaddr {
    type Output = Vaddr;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}
impl core::ops::AddAssign for Vaddr {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

/// Page tables entries
/// 63           54 53        28 27        19 18        10 9   8 7 6 5 4 3 2 1 0
/// +---------------+------------+------------+------------+-----+-+-+-+-+-+-+-+-+
/// |    Reserved   | PPN[2]     | PPN[1]     | PPN[0]     | RSW |D|A|G|U|X|W|R|V|
/// +---------------+------------+------------+------------+-----+-+-+-+-+-+-+-+-+

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PTE(u64);

impl PTE {
    pub fn new(ppn_2: u32, ppn_1: u32, ppn_0: u32, flags: u16) -> Self {
        Self(
            ((ppn_2 as u64) << 28)
                | ((ppn_1 as u64) << 19)
                | ((ppn_0 as u64) << 10)
                | (flags as u64),
        )
    }

    pub fn zero() -> Self {
        Self(0)
    }

    pub fn into_paddr(self, table: *mut PTE) -> *mut PTE {
        let addr = (self.0 as usize >> 10) << 12;
        table.with_addr(addr)
    }

    pub fn from_paddr(paddr: *mut u8) -> Self {
        Self(((paddr as u64) >> 12) << 10)
    }

    pub fn ppn_0(self) -> u32 {
        ((self.0 >> 10) & 0x1FF) as u32
    }

    pub fn ppn_1(self) -> u32 {
        ((self.0 >> 19) & 0x1FF) as u32
    }

    pub fn ppn_2(self) -> u32 {
        ((self.0 >> 28) & 0x3FF_FFFF) as u32
    }

    pub fn valid(self) -> bool {
        (self.0 & (1 << 0)) != 0
    }

    pub fn set_valid(self) -> Self {
        Self(self.0 | (1 << 0))
    }

    pub fn read(self) -> bool {
        (self.0 & (1 << 1)) != 0
    }

    pub fn write(self) -> bool {
        (self.0 & (1 << 2)) != 0
    }

    pub fn x(self) -> bool {
        (self.0 & (1 << 3)) != 0
    }

    pub fn u(self) -> bool {
        (self.0 & (1 << 4)) != 0
    }

    pub fn with_flags(self, flags: PageFlags) -> Self {
        Self(self.0 | flags.as_raw())
    }
}

#[derive(Default)]
pub struct PageFlags {
    read: bool,
    write: bool,
    execute: bool,
    user: bool,
}

impl PageFlags {
    pub fn kernel_all() -> Self {
        Self::default().read().write().execute()
    }

    pub fn all() -> Self {
        Self::default().read().write().execute().user()
    }

    pub fn read(mut self) -> Self {
        self.read = true;
        self
    }
    pub fn write(mut self) -> Self {
        self.write = true;
        self
    }
    pub fn execute(mut self) -> Self {
        self.execute = true;
        self
    }
    pub fn user(mut self) -> Self {
        self.user = true;
        self
    }

    fn as_raw(self) -> u64 {
        let mut flags = 0;
        flags |= if self.read { 1 } else { 0 } << 1;
        flags |= if self.write { 1 } else { 0 } << 2;
        flags |= if self.execute { 1 } else { 0 } << 3;
        flags |= if self.user { 1 } else { 0 } << 4;
        flags
    }
}

pub fn alloc_pages(n: usize) -> *mut u8 {
    unsafe {
        allocator::GLOBAL_ALLOCATOR
            .alloc_zeroed(Layout::from_size_align(PAGE_SIZE * n, PAGE_SIZE).unwrap())
    }
}

unsafe fn walk(mut pagetable: *mut PTE, vaddr: Vaddr, alloc: bool) -> Option<*mut PTE> {
    unsafe {
        for level in (1..=2).rev() {
            let index = vaddr.pt_index_for_level(level);
            let pte = pagetable.add(index);
            if (*pte).valid() {
                pagetable = (*pte).into_paddr(pagetable);
            } else if alloc {
                pagetable = allocator::GLOBAL_ALLOCATOR
                    .alloc_zeroed(Layout::from_size_align(PAGE_SIZE, PAGE_SIZE).unwrap())
                    as *mut PTE;
                *pte = PTE::from_paddr(pagetable as *mut u8).set_valid();
            } else {
                return None;
            }
        }
        Some(pagetable.add(vaddr.pt_index_for_level(0)))
    }
}

pub fn map_page(table1: *mut PTE, vaddr: Vaddr, paddr: Paddr, flags: PageFlags) {
    if !vaddr.is_aligned() {
        panic!("Virtual address not page-aligned");
    }
    if !is_aligned(paddr.0 as u64) {
        panic!("Physical address not page-aligned");
    }

    unsafe {
        match walk(table1, vaddr, true) {
            Some(pte) if (*pte).valid() => panic!("remap"),
            Some(pte) => *pte = PTE::from_paddr(paddr.0).with_flags(flags).set_valid(),
            None => unreachable!(),
        }
    }
}

fn is_aligned(value: u64) -> bool {
    value % 4096 == 0
}
