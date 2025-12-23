ENTRY(start)

SECTIONS {
    . = 0x1000000;

    .text :{
        KEEP(*(.text.start));
        *(.text .text.*);
    }

    .rodata : ALIGN(4) {
        *(.rodata .rodata.*);
    }

    .data : ALIGN(4) {
        *(.data .data.*);
    }

    .bss : ALIGN(4) {
        *(.bss .bss.* .sbss .sbss.*);
        . = ALIGN(16);
        . += 64 * 1024; /* 64 Kb */
        __stack_top = .;

        ASSERT(. < 0x1800000, "executable too large");
    }

    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)
    }
}
