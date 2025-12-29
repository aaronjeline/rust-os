# tarfile

A `no_std` TAR archive parser for extracting files from TAR archives.

Built with [nom](https://github.com/rust-bakery/nom) parser combinators, supporting basic USTAR format TAR files.

## Usage

```rust
use tarfile::tar_file;

let archive_data: &[u8] = /* your tar data */;
let (_, files) = tar_file::<()>().parse(archive_data).unwrap();

for file in files {
    println!("File: {}", file.header.name);
    println!("Size: {} bytes", file.header.file_size);
    // Access file.data
}
```
