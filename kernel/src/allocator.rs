use core::alloc::{GlobalAlloc, Layout};

use crate::println;

struct Mutable {
    next: usize,
    end: usize,
}

#[derive(Default)]
pub struct BumpAllocator {
    mutable: spin::Mutex<Option<Mutable>>,
}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            mutable: spin::Mutex::new(None),
        }
    }

    pub fn init(&self, start: *mut u8, end: *mut u8) {
        self.mutable.lock().replace(Mutable {
            next: start as usize,
            end: end as usize,
        });
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut lock = self.mutable.lock();
        let mutable = lock.as_mut().expect("Allocator not initialized");

        let addr = mutable.next.next_multiple_of(layout.align());
        assert!(
            addr.saturating_add(layout.size()) <= mutable.end,
            "Out of Memory!"
        );

        mutable.next = addr + layout.size();
        addr as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        println!("Dealloc called lol");
    }
}

#[global_allocator]
pub static GLOBAL_ALLOCATOR: BumpAllocator = BumpAllocator::new();
