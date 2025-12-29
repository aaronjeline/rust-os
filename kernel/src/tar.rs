use core::fmt::Error;

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{slice, vec};
use common::oct2int;
use nom::bytes::complete::take;
use nom::bytes::complete::{is_not, tag};
use nom::combinator::{map, verify};
use nom::error::ParseError;
use nom::multi::many;
use nom::sequence::{self, preceded, terminated};
use nom::{IResult, Parser};

use crate::memory::align_up;
use crate::println;
use crate::virtio::{BlockDeviceDriver, IOError, SECTOR_SIZE};
use tarfile::{FileRef, tar_file};
const FILES_MAX: usize = 2;

const DISK_MAX_SIZE: usize = align_up(1024 * 5, SECTOR_SIZE as usize);

pub struct File {
    in_use: bool,
    name: String,
    data: Vec<u8>,
}

impl<'a> From<tarfile::FileRef<'a>> for File {
    fn from(value: tarfile::FileRef<'a>) -> Self {
        Self {
            in_use: true,
            name: value.header.name.to_string(),
            data: value.data.to_vec(),
        }
    }
}

impl core::fmt::Debug for File {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "file: {}, size={}", self.name(), self.size())
    }
}

impl File {
    pub fn name(&'_ self) -> &str {
        &self.name
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

fn strcpy(src: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(src.len());
    for c in src {
        if *c == b'\0' {
            break;
        }
        v.push(*c);
    }
    v
}

pub struct BlockDevice<'driver> {
    disk: Vec<u8>,
    driver: &'driver mut BlockDeviceDriver,
}

impl<'driver> BlockDevice<'driver> {
    pub fn init(driver: &'driver mut BlockDeviceDriver) -> Result<Self, IOError> {
        let mut disk = vec![0; driver.capacity as usize];
        for sector in 0..disk.len() / SECTOR_SIZE as usize {
            driver.disk_read(&mut disk[sector * SECTOR_SIZE as usize..], sector as u64)?;
        }
        Ok(BlockDevice { disk, driver })
    }
}

pub struct FileSystem<'block_dev, 'driver> {
    dev: &'block_dev BlockDevice<'driver>,
    files: Vec<FileRef<'block_dev>>,
}

impl<'driver, 'block_dev> FileSystem<'block_dev, 'driver> {
    pub fn init(dev: &'block_dev BlockDevice<'driver>) -> Result<Self, FSError> {
        let (_, files) = tar_file::<nom::error::Error<&'_ [u8]>>().parse(&dev.disk)?;
        let files = files
            .into_iter()
            .map(|file_ref| file_ref.into())
            .collect::<Vec<_>>();
        Ok(Self { dev, files })
    }
}

#[derive(Debug)]
pub enum FSError {
    IOError(IOError),
    Parsing(String),
}

impl From<IOError> for FSError {
    fn from(value: IOError) -> Self {
        Self::IOError(value)
    }
}

impl<'a> From<nom::Err<nom::error::Error<&'a [u8]>>> for FSError {
    fn from(value: nom::Err<nom::error::Error<&'a [u8]>>) -> Self {
        Self::Parsing(value.to_string())
    }
}
