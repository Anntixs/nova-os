# Terminal input and output

## IMPLEMENTED

PS/2 IRQs enqueue key events. The scheduler's host loop drains events into a single canonical TTY line buffer. Typed characters are echoed to the Nova framebuffer terminal and COM1; Enter commits a line, waking one blocked stdin reader. User `READ(0,...)` sleeps through the scheduler if no complete line exists. `WRITE(1,...)` and `WRITE(2,...)` render through the same terminal. Thus `/bin/nsh` never reads keyboard ports or maps the framebuffer. TTY state is one-CPU/one-terminal and guarded by interrupt masking. Backspace edits the uncommitted line.

## PLANNED

Multiple TTYs, PTYs, raw mode, terminal signals, Unicode input and serial input as a TTY source. The current line buffer is 256 bytes and overlength keystrokes are ignored.
