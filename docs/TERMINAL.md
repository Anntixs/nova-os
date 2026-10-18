# Terminal and shell

## IMPLEMENTED

The UEFI GOP buffer remains Nova's display surface. A small built-in 5x7 font renders bootstrap text and a framebuffer terminal with newlines, backspace, screen clear, and pixel-copy scrolling. The shell uses a fixed 128-byte line buffer and a static command registry. PS/2 events reach it through a bounded input queue; commands never run inside IRQ handlers. `cpu`, `mem`, and `uptime` read CPUID, allocator statistics, and PIT ticks respectively. `run` and direct executable commands load an ELF through VFS from FAT32; `ps` reads the process manager. `ls`, `cd`, `pwd`, `cat`, `touch`, `mkdir`, `rm` and `write` use the same VFS. Terminal scrolling is confined below the boot header until `clear` uses the full screen.

## LIMITATIONS

The font renders lowercase as uppercase. There is no cursor glyph, history, selection, text wrapping beyond terminal width, or filesystem command. The shell is kernel-resident; userspace shell and VFS require later milestones.
