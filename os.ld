ENTRY(boot)

SECTIONS {
    . = 0x80200000;
    __kernel_start = .;

    .text :{
        KEEP(*(.text.boot));  // Put `boot` at the beginning
    }

    .text.stvec : ALIGN(4) {
        *(.text.stvec)
    }

    .text.rest : {
        *(.text .text.*);
    }

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*);
    }

    .data : ALIGN(8) {
        *(.data .data.*);
    }

    .bss : ALIGN(8) {
        __bss = .; 
        *(.bss .bss.* .sbss .sbss.*);
        __bss_end = .;
    }

    . = ALIGN(16);
    . += 1024*1024; /* 1MB */
    __stack_top = .;

    __heap = .;
    . += 100 * 1024 * 1024; /* 100MB */
    __heap_end = .;


    /DISCARD/ : {
        *(.eh_frame)
    }
}