#!/bin/bash

set -e

RUSTFLAGS="-C link-args=-Tuser.ld -C linker=rust-lld" \
    cargo build --bin shell --target riscv64gc-unknown-none-elf

cp ./target/riscv64gc-unknown-none-elf/debug/shell ./shell.elf

llvm-objcopy --set-section-flags .bss=alloc,contents -O binary \
      ./target/riscv64gc-unknown-none-elf/debug/shell shell.bin

RUSTFLAGS="-C link-args=-Tos.ld -C linker=rust-lld" \
    cargo build --bin kernel --target riscv64gc-unknown-none-elf

cp ./target/riscv64gc-unknown-none-elf/debug/kernel ./kernel.elf

