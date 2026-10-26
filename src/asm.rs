use core::arch::asm;

/// Make code written through the data cache visible to instruction fetches.
pub fn sync_instruction_cache(bytes: &[u8]) {
    let ctr: u64;
    unsafe {
        asm!("mrs {ctr}, ctr_el0", ctr = out(reg) ctr, options(nomem, nostack));
    }

    let data_line_size = 4usize << ((ctr >> 16) & 0xf);
    let instruction_line_size = 4usize << (ctr & 0xf);
    let start = bytes.as_ptr() as usize;
    let end = start + bytes.len();

    for address in (start & !(data_line_size - 1)..end).step_by(data_line_size) {
        unsafe {
            asm!("dc cvau, {address}", address = in(reg) address, options(nostack));
        }
    }
    unsafe {
        asm!("dsb ish", options(nostack));
    }

    for address in
        (start & !(instruction_line_size - 1)..end).step_by(instruction_line_size)
    {
        unsafe {
            asm!("ic ivau, {address}", address = in(reg) address, options(nostack));
        }
    }
    unsafe {
        asm!("dsb ish", "isb", options(nostack));
    }
}
