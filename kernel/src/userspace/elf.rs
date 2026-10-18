use crate::memory::{
    address_space::{AddressSpace, USER_BASE, USER_END, USER_STACK_TOP},
    paging::PageFlags,
};

fn u16at(b: &[u8], i: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(
        b.get(i..i + 2).ok_or("ELF truncated")?.try_into().unwrap(),
    ))
}
fn u32at(b: &[u8], i: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(
        b.get(i..i + 4).ok_or("ELF truncated")?.try_into().unwrap(),
    ))
}
fn u64at(b: &[u8], i: usize) -> Result<u64, &'static str> {
    Ok(u64::from_le_bytes(
        b.get(i..i + 8).ok_or("ELF truncated")?.try_into().unwrap(),
    ))
}
pub fn load(image: &[u8], space: &AddressSpace, nx: bool) -> Result<u64, &'static str> {
    if image.len() < 64
        || &image[..4] != b"\x7fELF"
        || image[4] != 2
        || image[5] != 1
        || image[6] != 1
        || u16at(image, 16)? != 2
        || u16at(image, 18)? != 62
        || u16at(image, 52)? != 64
    {
        return Err("unsupported ELF64 x86_64 executable");
    }
    let entry = u64at(image, 24)?;
    let off = usize::try_from(u64at(image, 32)?).map_err(|_| "PHDR offset overflow")?;
    let size = u16at(image, 54)? as usize;
    let count = u16at(image, 56)? as usize;
    if size < 56
        || count == 0
        || off
            .checked_add(size.checked_mul(count).ok_or("PHDR overflow")?)
            .filter(|&end| end <= image.len())
            .is_none()
    {
        return Err("PHDR out of range");
    }
    let mut executable_entry = false;
    for i in 0..count {
        let p = off + i * size;
        let kind = u32at(image, p)?;
        if kind == 2 || kind == 3 {
            return Err("dynamic ELF is unsupported");
        }
        if kind != 1 {
            continue;
        }
        let flags = u32at(image, p + 4)?;
        if flags & !7 != 0 || flags & 3 == 3 {
            return Err("unsupported or WX ELF segment");
        }
        let file_off = usize::try_from(u64at(image, p + 8)?).map_err(|_| "file offset overflow")?;
        let va = u64at(image, p + 16)?;
        let filesz = u64at(image, p + 32)?;
        let memsz = u64at(image, p + 40)?;
        let align = u64at(image, p + 48)?;
        let file_len = usize::try_from(filesz).map_err(|_| "segment file size overflow")?;
        if memsz < filesz
            || memsz == 0
            || va < USER_BASE
            || va.checked_add(memsz).filter(|&v| v <= USER_END).is_none()
            || (align > 1 && (!align.is_power_of_two() || va % align != file_off as u64 % align))
            || (va < USER_STACK_TOP && va.checked_add(memsz).is_some_and(|end|end > USER_STACK_TOP-8192))
            || file_off
                .checked_add(file_len)
                .filter(|&v| v <= image.len())
                .is_none()
        {
            return Err("invalid ELF segment bounds");
        }
        if va & 4095 != file_off as u64 & 4095 {
            return Err("ELF segment alignment mismatch");
        }
        if flags & 1 != 0 && entry >= va && entry < va + memsz {
            executable_entry = true;
        }
        let mut page = va & !4095;
        let end = (va + memsz + 4095) & !4095;
        let mut bits = PageFlags::USER.0;
        if flags & 2 != 0 {
            bits |= PageFlags::WRITABLE.0;
        }
        if flags & 1 == 0 && nx {
            bits |= PageFlags::NO_EXECUTE.0;
        }
        while page < end {
            space.map_user(page, PageFlags(bits))?;
            page += 4096;
        }
        crate::nova_log!(
            Debug,
            "ELF",
            "mapped LOAD at {:#x}, {} bytes, flags {:#x}",
            va,
            memsz,
            flags
        );
        for j in 0..filesz {
            let phys = space
                .translate_user(va + j, false)
                .ok_or("segment translation failed")?;
            // SAFETY: loader exclusively owns the freshly allocated, identity-mapped frame.
            unsafe {
                (phys as *mut u8).write(image[file_off + j as usize]);
            }
        }
    }
    if !executable_entry {
        return Err("ELF entry outside executable segment");
    }
    for page in [USER_STACK_TOP - 8192, USER_STACK_TOP - 4096] {
        let mut flags = PageFlags::USER.0 | PageFlags::WRITABLE.0;
        if nx {
            flags |= PageFlags::NO_EXECUTE.0;
        }
        space.map_user(page, PageFlags(flags))?;
    }
    Ok(entry)
}

#[cfg(feature = "selftest")]
pub fn selftest(info: &nova_core::NovaBootInfo, nx: bool) {
    use crate::memory::physical;
    use alloc::vec::Vec;
    let before = physical::stats().free_bytes;
    let image = crate::fs::vfs::read("/bin/hello").expect("filesystem hello");
    let space = AddressSpace::new().expect("ELF test address space");
    assert!(load(&image[..32], &space, nx).is_err());
    let mut malformed = Vec::from(image.as_slice());
    malformed[0] = 0;
    assert!(load(&malformed, &space, nx).is_err());
    malformed[0] = 0x7f;
    malformed[18] = 0;
    assert!(load(&malformed, &space, nx).is_err());
    malformed[18] = 62;
    let phoff = u64at(&image, 32).unwrap() as usize;
    malformed[phoff + 4] = 7; // W+X segment
    assert!(load(&malformed, &space, nx).is_err());
    malformed=Vec::from(image.as_slice());
    malformed[phoff+48..phoff+56].copy_from_slice(&3u64.to_le_bytes());
    assert!(load(&malformed,&space,nx).is_err());
    malformed=Vec::from(image.as_slice());
    malformed[phoff+40..phoff+48].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(load(&malformed,&space,nx).is_err());
    malformed=Vec::from(image.as_slice());
    malformed[24..32].copy_from_slice(&0u64.to_le_bytes());
    // A late ELF validation failure can leave mapped pages in this address space;
    // production callers discard that space. Test the same ownership rule here.
    let rejected = AddressSpace::new().expect("rejected ELF address space");
    assert!(load(&malformed,&rejected,nx).is_err());
    rejected.destroy(info).expect("rejected ELF cleanup");
    let entry = load(&image, &space, nx).expect("valid hello ELF");
    assert_eq!(entry, USER_BASE);
    assert!(space.translate_user(entry, false).is_some());
    assert!(space.translate_user(entry, true).is_none());
    assert!(space.translate_user(USER_STACK_TOP - 8, true).is_some());
    space.destroy(info).expect("ELF test cleanup");
    assert_eq!(physical::stats().free_bytes, before);
    crate::log("[NOVA][TEST] ELF validation and mapping OK");
}
