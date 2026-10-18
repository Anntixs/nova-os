ENTRY(_start)
PHDRS { text PT_LOAD FLAGS(5); data PT_LOAD FLAGS(6); }
SECTIONS {
  . = 0x0000400000000000;
  .text : { *(.text .text.*) } :text
  .rodata : { *(.rodata .rodata.*) } :text
  . = ALIGN(0x1000);
  .data : { *(.data .data.*) *(.bss .bss.*) } :data
  /DISCARD/ : { *(.eh_frame*) *(.note*) *(.comment*) }
}
