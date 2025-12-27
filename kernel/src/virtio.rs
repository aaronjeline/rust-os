use core::{
    marker::PhantomPinned,
    mem::offset_of,
    pin::Pin,
    ptr::{addr_of, addr_of_mut, copy_nonoverlapping},
    sync::atomic::{Ordering, fence},
};

use alloc::boxed::Box;

use crate::{
    memory::{PAGE_SIZE, align_up, alloc_pages},
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

#[derive(Debug, Clone, Copy, Default)]
#[repr(packed)]
pub struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(packed)]
pub struct Available {
    flags: u16,
    index: u16,
    ring: [u16; VIRTQ_ENTRY_NUM],
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(packed)]
pub struct UsedElemEntry {
    id: u32,
    len: u32,
}

#[repr(packed)]
#[derive(Default, Clone, Copy)]
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
    used_index: *mut u16, // Points to used.index
    last_used_index: u16,
    __phantom: PhantomPinned, // Ensures this structure is pinned, as it points to itself
}

impl Virtq {
    pub fn init(index: u32) -> Pin<Box<Self>> {
        // Can't use `Box::new()` here as we need it to be page-aligned
        let virtq_paddr = alloc_pages(align_up(size_of::<Virtq>(), PAGE_SIZE) / PAGE_SIZE);
        let mut ptr = unsafe { Box::from_raw(virtq_paddr as *mut Virtq) };
        (*ptr).queue_index = index;
        (*ptr).used_index = addr_of_mut!((*ptr).used.index);
        unsafe {
            virtio_reg_write32(VIRTIO_REG_QUEUE_SEL, index);
            virtio_reg_write32(VIRTIO_REG_QUEUE_NUM, VIRTQ_ENTRY_NUM as u32);
            virtio_reg_write32(VIRTIO_REG_QUEUE_ALIGN, 0);
            // Write the physical address of the virtq to
            virtio_reg_write64(VIRTIO_REG_QUEUE_PFN, addr_of!(*ptr).addr() as u64);
        }
        Box::into_pin(ptr)
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

impl BlockRequest {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
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

#[derive(Debug, Clone)]
pub enum IOError {
    InvalidSector(u64, u64),
    NotEnoughSpaceForRead(usize),
    WriteFail(u64, u8),
    ReadFail(u64, u8),
}

impl core::fmt::Display for IOError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IOError::InvalidSector(sector, capacity) => write!(
                f,
                "Tried to write to sector {sector} but capacity was {capacity}"
            ),
            IOError::WriteFail(sector, status) => {
                write!(f, "Failed to write to sector {sector}, status: {status}")
            }
            IOError::ReadFail(sector, status) => {
                write!(f, "Failed to read from sector {sector}, status: {status}")
            }
            IOError::NotEnoughSpaceForRead(size) => write!(
                f,
                "Buffer did not have enough space to read a sector. Had {size} bytes, must have at least {SECTOR_SIZE} bytes."
            ),
        }
    }
}

impl core::error::Error for IOError {}

pub struct BlockDeviceDriver {
    pub virtq: Pin<Box<Virtq>>,
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
        }

        // let virtq = virtq_init(0);
        let virtq = Virtq::init(0);
        let capacity;

        unsafe {
            virtio_reg_write32(VIRTIO_REG_DEVICE_STATUS, VIRTIO_STATUS_DRIVER_OK);
            // Get the disk capacity
            capacity = virtio_reg_read64(VIRTIO_REG_DEVICE_CONFIG + 0) * SECTOR_SIZE;
        }
        println!("virtio-blk: capacity is {capacity} bytes");
        Self { capacity, virtq }
    }

    /// SAFETY: DO NOT CALL std::mem::swap() on this pointer
    fn vq(&mut self) -> &mut Virtq {
        unsafe { Pin::get_unchecked_mut(self.virtq.as_mut()) }
    }

    /// Notify the device of a new request
    pub fn kick(&mut self, desc_index: u16) {
        let index = ((*self.virtq).available.index as usize) % VIRTQ_ENTRY_NUM;
        self.vq().available.ring[index] = desc_index;
        self.vq().available.index += 1;
        fence(Ordering::SeqCst);
        unsafe {
            virtio_reg_write32(VIRTIO_REG_QUEUE_NOTIFY, (*self.virtq).queue_index);
        }
        self.vq().last_used_index += 1;
    }

    /// Are there any requests being processed device side?
    pub fn is_busy(&self) -> bool {
        unsafe { (*self.virtq).last_used_index != *(*self.virtq).used_index }
    }

    pub fn disk_read(&mut self, buf: &mut [u8], sector: u64) -> Result<(), IOError> {
        if buf.len() < SECTOR_SIZE as usize {
            return Err(IOError::NotEnoughSpaceForRead(buf.len()));
        }
        self.assert_sector_in_range(sector)?;
        let mut request = BlockRequest::default();
        request.sector = sector;
        request.type_ = VIRTIO_BLK_T_IN;
        let address = addr_of!(request).addr();
        self.vq().descriptors[0].addr = address as u64;
        self.vq().descriptors[0].len = (size_of::<u32>() * 2 + size_of::<u64>()) as u32;
        self.vq().descriptors[0].flags = VIRTQ_DESC_F_NEXT;
        self.vq().descriptors[0].next = 1;

        self.vq().descriptors[1].addr = (address + offset_of!(BlockRequest, data)) as u64;
        self.vq().descriptors[1].len = SECTOR_SIZE as u32;
        self.vq().descriptors[1].flags = VIRTQ_DESC_F_NEXT | VIRTQ_DESC_F_WRITE;
        self.vq().descriptors[1].next = 2;

        self.vq().descriptors[2].addr = (address + offset_of!(BlockRequest, status)) as u64;
        self.vq().descriptors[2].len = size_of::<u8>() as u32;
        self.vq().descriptors[2].flags = VIRTQ_DESC_F_WRITE;

        self.kick(0);
        while self.is_busy() {}
        if !request.ok() {
            return Err(IOError::ReadFail(sector, request.status));
        }
        unsafe {
            copy_nonoverlapping(
                request.data.as_ptr(),
                buf.as_mut_ptr(),
                SECTOR_SIZE as usize,
            );
        }
        Ok(())
    }

    fn assert_sector_in_range(&self, sector: u64) -> Result<(), IOError> {
        if sector < self.capacity / SECTOR_SIZE {
            Ok(())
        } else {
            Err(IOError::InvalidSector(sector, self.capacity))
        }
    }

    pub fn disk_write(&mut self, buf: &[u8], sector: u64) -> Result<(), IOError> {
        self.assert_sector_in_range(sector)?;
        let mut request = BlockRequest::default();
        request.sector = sector;
        request.type_ = VIRTIO_BLK_T_OUT;
        unsafe {
            copy_nonoverlapping(buf.as_ptr(), request.data.as_mut_ptr(), buf.len());
        }
        let address = addr_of!(request).addr();
        self.vq().descriptors[0].addr = address as u64;
        self.vq().descriptors[0].len = (size_of::<u32>() * 2 + size_of::<u64>()) as u32;
        self.vq().descriptors[0].flags = VIRTQ_DESC_F_NEXT;
        self.vq().descriptors[0].next = 1;

        self.vq().descriptors[1].addr = (address + offset_of!(BlockRequest, data)) as u64;
        self.vq().descriptors[1].len = SECTOR_SIZE as u32;
        self.vq().descriptors[1].flags = VIRTQ_DESC_F_NEXT;
        self.vq().descriptors[1].next = 2;

        self.vq().descriptors[2].addr = (address + offset_of!(BlockRequest, status)) as u64;
        self.vq().descriptors[2].len = size_of::<u8>() as u32;
        self.vq().descriptors[2].flags = VIRTQ_DESC_F_WRITE;

        self.kick(0);
        while self.is_busy() {}

        if request.ok() {
            Ok(())
        } else {
            Err(IOError::WriteFail(sector, request.status))
        }
    }
}
