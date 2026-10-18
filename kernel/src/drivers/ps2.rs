use crate::{
    arch::x86_64::cpu,
    input::{self, KeyEvent, ALT, CAPS, CTRL, SHIFT},
};
static mut MODIFIERS: u8 = 0;
static mut EXTENDED: bool = false;
fn wait_input() -> bool {
    for _ in 0..100_000 {
        if cpu::port_in(0x64) & 2 == 0 {
            return true;
        }
    }
    false
}
fn wait_output() -> bool {
    for _ in 0..100_000 {
        if cpu::port_in(0x64) & 1 != 0 {
            return true;
        }
    }
    false
}
pub fn init() -> Result<(), ()> {
    if !wait_input() {
        return Err(());
    }
    cpu::port_out(0x64, 0x20);
    if !wait_output() {
        return Err(());
    }
    let mut config = cpu::port_in(0x60);
    config = (config | 1) & !0x10;
    if !wait_input() {
        return Err(());
    }
    cpu::port_out(0x64, 0x60);
    if !wait_input() {
        return Err(());
    }
    cpu::port_out(0x60, config);
    while cpu::port_in(0x64) & 1 != 0 {
        let _ = cpu::port_in(0x60);
    }
    Ok(())
}
fn ascii(code: u8, shift: bool, caps: bool) -> u8 {
    let normal = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./";
    let shifted = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?";
    if code == 0x39 {
        return b' ';
    }
    let i = code as usize;
    if i >= normal.len() {
        return 0;
    }
    let c = normal[i];
    if c.is_ascii_alphabetic() {
        if shift ^ caps {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        }
    } else if shift {
        shifted[i]
    } else {
        c
    }
}
pub fn on_irq() {
    let scan = cpu::port_in(0x60);
    if scan == 0xe0 {
        // SAFETY: IRQ1 is the sole writer of EXTENDED.
        unsafe {
            EXTENDED = true;
        }
        return;
    }
    // SAFETY: IRQ1 is the sole writer of modifier state; current IRQ handler is not reentrant.
    unsafe {
        if EXTENDED {
            EXTENDED = false;
            return;
        }
        let released = scan & 0x80 != 0;
        let code = scan & 0x7f;
        match code {
            0x2a | 0x36 => {
                if released {
                    MODIFIERS &= !SHIFT
                } else {
                    MODIFIERS |= SHIFT
                }
            }
            0x1d => {
                if released {
                    MODIFIERS &= !CTRL
                } else {
                    MODIFIERS |= CTRL
                }
            }
            0x38 => {
                if released {
                    MODIFIERS &= !ALT
                } else {
                    MODIFIERS |= ALT
                }
            }
            0x3a => {
                if !released {
                    MODIFIERS ^= CAPS
                }
            }
            _ => {}
        }
        let key = ascii(code, MODIFIERS & SHIFT != 0, MODIFIERS & CAPS != 0);
        input::push(KeyEvent {
                key,
                pressed: !released,
                modifiers: MODIFIERS,
                code: code as u16,
                timestamp_ms: crate::interrupts::timer::uptime_ms(),
        });
    }
}
