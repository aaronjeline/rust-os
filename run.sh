#!/bin/bash

set -xue

OBJCOPY=/opt/homebrew/opt/llvm/bin/llvm-objcopy

./build.sh


QEMU=qemu-system-riscv64


#$OBJCOPY --set-section-flags .bss=alloc,contents -O binary shell.elf shell.bin
#$OBJCOPY -Ibinary -Oelf64-littleriscv shell.bin shell.bin.o



$QEMU -machine virt \
    -bios default \
    -cpu rv64 \
    -nographic \
    -smp 1 \
    -m 128M \
    -d cpu_reset,unimp,guest_errors,int -D qemu.og \
    -serial mon:stdio \
    -drive id=drive0,file=lorem.txt,format=raw,if=none \
    -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0 \
    --no-reboot \
    -kernel ./kernel.elf
