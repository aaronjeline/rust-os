use core::{
    mem::offset_of,
    ptr::{addr_of, addr_of_mut, copy_nonoverlapping},
    sync::atomic::{Ordering, fence},
};

use crate::{
    memory::{PAGE_SIZE, alloc_pages},
    println,
};

const SECTOR_SIZE: u64 = 512;

const VIRTQ_ENTRY_NUM: usize = 16;
const VIRTIO_DEVICE_BLK: u32 = 2;
pub const VIRTIO_BLK_PADDR: u64 = 0x10001000;
const VIRTIO_REG_MAGIC: usize = 0x00;
const VIRTIO_REG_VERSION: usize = 0x04;
const VIRTIO_REG_DEVICE_ID: usize = 0x08;
const VIRTIO_REG_QUEUE_SEL: usize = 0x30;
const VIRTIO_REG_QUEUE_NUM_MAX: usize = 0x34;
const VIRTIO_REG_QUEUE_NUM: usize = 0x38;
const VIRTIO_REG_QUEUE_ALIGN: usize = 0x3c;
const VIRTIO_REG_QUEUE_PFN: usize = 0x40;
const VIRTIO_REG_QUEUE_READY: usize = 0x44;
const VIRTIO_REG_QUEUE_NOTIFY: usize = 0x50;
const VIRTIO_REG_DEVICE_STATUS: usize = 0x70;
const VIRTIO_REG_DEVICE_CONFIG: usize = 0x100;
const VIRTIO_STATUS_ACK: u32 = 1;
const VIRTIO_STATUS_DRIVER: u32 = 2;
const VIRTIO_STATUS_DRIVER_OK: u32 = 4;
const VIRTIO_STATUS_FEAT_OK: u32 = 8;
const VIRTQ_DESC_F_NEXT: u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;
const VIRTQ_AVAIL_F_NO_INTERRUPT: usize = 1;
const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

#[derive(Debug, Clone)]
#[repr(packed)]
pub struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[derive(Debug, Clone)]
#[repr(packed)]
pub struct Available {
    flags: u16,
    index: u16,
    ring: [u16; VIRTQ_ENTRY_NUM],
}

#[derive(Debug, Clone)]
#[repr(packed)]
pub struct UsedElemEntry {
    id: u32,
    len: u32,
}

#[repr(packed)]
pub struct Used {
    flags: u16,
    index: u16,
    ring: [UsedElemEntry; VIRTQ_ENTRY_NUM],
}

#[repr(packed)]
pub struct Virtq {
    descriptors: [Descriptor; VIRTQ_ENTRY_NUM],
    available: Available,
    padding: [u8; PAGE_SIZE
        - ((size_of::<[Descriptor; VIRTQ_ENTRY_NUM]>() + size_of::<Available>()) % PAGE_SIZE)],
    used: Used,
    queue_index: u32,
    used_index: *mut u16,
    last_used_index: u16,
}

fn align_up(value: usize, align: usize) -> usize {
    if value % align == 0 {
        value
    } else {
        let rem = value % align;
        let slop = align - rem;
        value + slop
    }
}

#[derive(Debug, Clone)]
#[repr(packed)]
struct BlockRequest {
    type_: u32,
    reserved: u32,
    sector: u64,
    data: [u8; 512],
    status: u8,
}

impl Default for BlockRequest {
    fn default() -> Self {
        Self {
            type_: 0,
            reserved: 0,
            sector: 0,
            data: [0; 512],
            status: 0,
        }
    }
}

unsafe fn virtio_reg_read32(offset: usize) -> u32 {
    unsafe {
        let addr = (VIRTIO_BLK_PADDR as *mut u8).add(offset) as *mut u32;
        addr.read_volatile()
    }
}

unsafe fn virtio_reg_write32(offset: usize, value: u32) {
    unsafe {
        let addr = (VIRTIO_BLK_PADDR as *mut u8).add(offset) as *mut u32;
        addr.write_volatile(value);
    }
}

unsafe fn virtio_reg_read64(offset: usize) -> u64 {
    unsafe {
        let addr = (VIRTIO_BLK_PADDR as *mut u8).add(offset) as *mut u64;
        addr.read_volatile()
    }
}

unsafe fn virtio_reg_write64(offset: usize, value: u64) {
    unsafe {
        let addr = (VIRTIO_BLK_PADDR as *mut u8).add(offset) as *mut u64;
        addr.write_volatile(value);
    }
}

unsafe fn virtio_reg_fetch_and_or32(offset: usize, value: u32) {
    unsafe {
        let addr = (VIRTIO_BLK_PADDR as *mut u8).add(offset) as *mut u32;
        addr.write_volatile(addr.read_volatile() | value);
    }
}

pub struct BlockDeviceDriver {
    pub virtq: *mut Virtq,
    pub block_req: *mut BlockRequest,
    pub capacity: u64,
}

impl BlockDeviceDriver {
    /// Initialize the virtual block device
    pub fn new() -> Self {
        unsafe {
            match virtio_reg_read32(VIRTIO_REG_MAGIC) {
                0x74726976 => (),
                magic => panic!("virtio: Invalid magic, got {:#x}", magic),
            };
            match virtio_reg_read32(VIRTIO_REG_VERSION) {
                1 => (),
                version => panic!("virtio: Invalid version, got {version}"),
            };
            match virtio_reg_read32(VIRTIO_REG_DEVICE_ID) {
                VIRTIO_DEVICE_BLK => (),
                other => panic!("virtio: Invalid device: {other}"),
            }
            println!("virtio: Sanity checks passed!");

            // Reset the device
            virtio_reg_write32(VIRTIO_REG_DEVICE_STATUS, 0);
            // Set the ACKNOWLEDGE status bit
            virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_ACK);
            // Set the DRIVER status bit
            virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_DRIVER);
            // Set the FEATURES_OK status bit.
            // (Nominally, we should scan the offered features and make sure we can handle them, but 🤷‍♂️)
            virtio_reg_fetch_and_or32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_FEAT_OK);

            let virtq = virtq_init(0);
            virtio_reg_write32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_DRIVER_OK);

            // Get the disk capacity
            let capacity = virtio_reg_read64(VIRTIO_REG_DEVICE_CONFIG + 0) * SECTOR_SIZE;
            println!("virtio-blk: capacity is {capacity} bytes");
            // Allocate a block request structure
            // FIXME: there's got a be a more rusty way to do this)
            let block_req = alloc_pages(align_up(size_of::<BlockRequest>(), PAGE_SIZE) / PAGE_SIZE)
                as *mut BlockRequest;
            Self {
                capacity,
                block_req,
                virtq,
            }
        }
    }

    /// Notify the device of a new request
    pub fn kick(&self, desc_index: u16) {
        unsafe {
            let index = ((*self.virtq).available.index as usize) % VIRTQ_ENTRY_NUM;
            (*self.virtq).available.ring[index] = desc_index;
            (*self.virtq).available.index += 1;
            fence(Ordering::SeqCst);
            virtio_reg_write32(VIRTIO_REG_QUEUE_NOTIFY, (*self.virtq).queue_index);
            (*self.virtq).last_used_index += 1;
        }
    }

    /// Are there any requests being processed device side?
    pub fn is_busy(&self) -> bool {
        unsafe { (*self.virtq).last_used_index != *(*self.virtq).used_index }
    }

    /// FIXME: this whole function is pretty much a straight C transliteration
    pub fn read_write_disk(&self, buf: &mut [u8], sector: u64, is_write: bool) {
        if sector >= self.capacity / SECTOR_SIZE {
            println!(
                "virtio: tried to read/write sector={sector}, but capacity is {}",
                self.capacity
            );
            return;
        }
        let mut request = BlockRequest::default();
        unsafe {
            request.sector = sector;
            request.type_ = if is_write {
                VIRTIO_BLK_T_OUT
            } else {
                VIRTIO_BLK_T_IN
            };
            if is_write {
                copy_nonoverlapping(buf.as_ptr(), request.data.as_mut_ptr(), buf.len());
            }
            let blk_req_paddr = addr_of!(request).addr();
            (*self.virtq).descriptors[0].addr = blk_req_paddr as u64;
            (*self.virtq).descriptors[0].len = (size_of::<u32>() * 2 + size_of::<u64>()) as u32;
            (*self.virtq).descriptors[0].flags = VIRTQ_DESC_F_NEXT;
            (*self.virtq).descriptors[0].next = 1;

            (*self.virtq).descriptors[1].addr =
                (blk_req_paddr + offset_of!(BlockRequest, data)) as u64;
            (*self.virtq).descriptors[1].len = SECTOR_SIZE as u32;
            (*self.virtq).descriptors[1].flags =
                VIRTQ_DESC_F_NEXT | (if is_write { 0 } else { VIRTQ_DESC_F_WRITE });
            (*self.virtq).descriptors[1].next = 2;

            (*self.virtq).descriptors[2].addr =
                (blk_req_paddr + offset_of!(BlockRequest, status)) as u64;
            (*self.virtq).descriptors[2].len = size_of::<u8>() as u32;
            (*self.virtq).descriptors[2].flags = VIRTQ_DESC_F_WRITE;
        }
        self.kick(0);
        while self.is_busy() {}

        if request.status != 0 {
            println!(
                "virtio: warn: failed to read/write sector={sector}, {}",
                request.status
            );
        }

        if !is_write {
            unsafe {
                // FIXME: probably need to check if buf is big enough
                copy_nonoverlapping(
                    request.data.as_ptr(),
                    buf.as_mut_ptr(),
                    SECTOR_SIZE as usize,
                );
            }
        }
    }
}

fn virtq_init(index: u32) -> *mut Virtq {
    let virtq_paddr = alloc_pages(align_up(size_of::<Virtq>(), PAGE_SIZE) / PAGE_SIZE);
    let vq = virtq_paddr as *mut Virtq;
    unsafe {
        (*vq).queue_index = index;
        (*vq).used_index = addr_of_mut!((*vq).used.index);
        virtio_reg_write32(VIRTIO_REG_QUEUE_SEL, index);
        virtio_reg_write32(VIRTIO_REG_QUEUE_NUM, VIRTQ_ENTRY_NUM as u32);
        virtio_reg_write32(VIRTIO_REG_QUEUE_ALIGN, 0);
        // Write the physical address of the virtq to
        virtio_reg_write64(VIRTIO_REG_QUEUE_PFN, virtq_paddr.addr() as u64);
    }
    vq
}
