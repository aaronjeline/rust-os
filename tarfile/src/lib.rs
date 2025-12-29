#![no_std]
extern crate alloc;

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;
use nom::bytes::complete::{is_not, tag, take};
use nom::combinator::{map, map_parser, verify};
use nom::error::ParseError;
use nom::multi::many_till;
use nom::sequence::terminated;
use nom::Parser;

const BLOCK_SIZE: usize = 512;

pub fn tar_file<'a, E>() -> impl Parser<&'a [u8], Output = Vec<FileRef<'a>>, Error = E>
where
    E: ParseError<&'a [u8]>,
{
    many_till(FileRef::parser(), zero_block()).map(|(files, ())| files)
}

fn zero_block<'a, E>() -> impl Parser<&'a [u8], Output = (), Error = E>
where
    E: ParseError<&'a [u8]>,
{
    tag([0; BLOCK_SIZE * 2].as_slice()).map(|_| ())
}

#[cfg(test)]
mod test {
    use super::*;

    static SINGLE_TAR: &[u8] = include_bytes!("../../common/single.tar");
    static DOUBLE_TAR: &[u8] = include_bytes!("../../common/double.tar");

    #[test]
    fn test_double() {
        let (_, files) = tar_file::<()>().parse(DOUBLE_TAR).expect("should parse");
        assert_eq!(files.len(), 2, "Should have two files");
        let file1 = &files[0];
        assert_eq!(file1.header.name, "./hello.txt");
        assert_eq!(file1.data, b"Hello!\n");
        let file2 = &files[1];
        assert_eq!(file2.header.name, "./meow.txt");
        assert_eq!(file2.data, b"Meow!\n");
    }

    #[test]
    fn test_single() {
        let (_, files) = tar_file::<()>().parse(SINGLE_TAR).expect("Should parse!");
        assert_eq!(files.len(), 1, "Should be one single file");
        let file = &files[0];
        assert_eq!(file.header.name, "./hello.txt");
        assert_eq!(file.data, b"Hello!\n");
    }
}

pub struct FileRef<'a> {
    pub header: TarHeader<'a>,
    pub data: &'a [u8],
}

impl<'a> FileRef<'a> {
    pub fn parser<E: ParseError<&'a [u8]>>() -> impl Parser<&'a [u8], Output = Self, Error = E> {
        TarHeader::parser().flat_map(Self::file_data)
    }

    pub fn file_data<E>(header: TarHeader<'a>) -> impl Parser<&'a [u8], Output = Self, Error = E>
    where
        E: ParseError<&'a [u8]>,
    {
        let needs_slop_block = header.file_size % BLOCK_SIZE != 0;
        let num_blocks = (header.file_size / BLOCK_SIZE) + if needs_slop_block { 1 } else { 0 };
        blocks(num_blocks).map(move |data| Self {
            header: header.clone(),
            data: &data[..header.file_size],
        })
    }
}

#[derive(Debug, Clone)]
pub struct TarHeader<'a> {
    pub name: Cow<'a, str>, // Offset 0
    pub file_size: usize,   // Offset 124
}

fn blocks<'a, E>(num: usize) -> impl Parser<&'a [u8], Output = &'a [u8], Error = E>
where
    E: ParseError<&'a [u8]>,
{
    take(num * BLOCK_SIZE)
}

fn block<'a, E>() -> impl Parser<&'a [u8], Output = &'a [u8], Error = E>
where
    E: ParseError<&'a [u8]>,
{
    take(BLOCK_SIZE)
}

impl<'a> TarHeader<'a> {
    pub fn parser<E>() -> impl Parser<&'a [u8], Output = Self, Error = E>
    where
        E: ParseError<&'a [u8]>,
    {
        map_parser(block(), Self::header_block())
    }

    pub fn header_block<E>() -> impl Parser<&'a [u8], Output = Self, Error = E>
    where
        E: ParseError<&'a [u8]>,
    {
        (
            null_terminated_padded(100), // 100 bytes
            skip(24),
            Self::file_size(), // 12 bytes
            skip(121),
            Self::magic(), // 8 bytes
            skip(235),
        )
            .map(|(name, (), file_size, (), (), ())| Self { name, file_size })
    }

    fn file_size<E>() -> impl Parser<&'a [u8], Output = usize, Error = E>
    where
        E: ParseError<&'a [u8]>,
    {
        take(12usize).map(|buf| common::oct2int(buf) as usize)
    }

    fn magic<E>() -> impl Parser<&'a [u8], Output = (), Error = E>
    where
        E: ParseError<&'a [u8]>,
    {
        map(verify(take(6usize), |buf: &[u8]| buf == b"ustar\0"), |_| ())
    }
}

fn null_terminated_padded<'a, E>(
    max_len: usize,
) -> impl Parser<&'a [u8], Output = Cow<'a, str>, Error = E>
where
    E: ParseError<&'a [u8]>,
{
    null_terminated()
        .flat_map(move |name| take(max_len - (name.len() + 1)).map(move |_| name.clone()))
}

fn skip<'a, E>(n: usize) -> impl Parser<&'a [u8], Output = (), Error = E>
where
    E: ParseError<&'a [u8]>,
{
    take(n).map(|_| ())
}

fn null_terminated<'a, E>() -> impl Parser<&'a [u8], Output = Cow<'a, str>, Error = E>
where
    E: ParseError<&'a [u8]>,
{
    terminated(is_not("\0"), tag("\0")).map(String::from_utf8_lossy)
}
