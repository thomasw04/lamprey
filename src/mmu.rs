use core::ops::Range;

use anyhow::Result;
use uefi::boot::{self, AllocateType, MemoryType, PAGE_SIZE};
use uefi::mem::memory_map::MemoryMap;

pub const MEM_TYPE_DRIVER: MemoryType = MemoryType::custom(0x8000_0000);
pub const MEM_TYPE_STACK: MemoryType = MemoryType::custom(0x8000_0001);
pub const MEM_TYPE_MULTIBOOT: MemoryType = MemoryType::custom(0x8000_0002);
pub const MEM_TYPE_ELF: MemoryType = MemoryType::custom(0x8000_0003);
pub const MEM_TYPE_TABLE: MemoryType = MemoryType::custom(0x8000_0004);
pub const MEM_TYPE_CORE_DATA: MemoryType = MemoryType::custom(0x8000_0005);
pub const RAM_FLAGS: u64 = (3 << 8) | (1 << 10); // Inner-shareable, accessed, MAIR index 0.
pub const DEVICE_FLAGS: u64 = RAM_FLAGS | (1 << 2); // MAIR index 1: Device-nGnRnE.

fn allocate_table() -> Result<&'static mut [u8]> {
    let addr = boot::allocate_pages(AllocateType::AnyPages, MEM_TYPE_TABLE, 1)?;
    // SAFETY: this is a fresh, exclusively owned page retained for the handoff.
    unsafe {
        addr.as_ptr().write_bytes(0, PAGE_SIZE);
        Ok(core::slice::from_raw_parts_mut(addr.as_ptr(), PAGE_SIZE))
    }
}

fn ensure_child_table(entry: &mut u64) -> Result<&mut [u8]> {
    if *entry & 1 == 0 {
        *entry = allocate_table()?.as_ptr() as u64 | 3;
    }
    // SAFETY: the descriptor points to a separately allocated table page.
    // Callers hold the parent exclusively and only pass table descriptors.
    Ok(unsafe {
        core::slice::from_raw_parts_mut((*entry & 0x0000_ffff_ffff_f000) as *mut u8, PAGE_SIZE)
    })
}

/// Note: This only ensures that the range this L3 table covers is mapped.
fn ensure_l3_table(table: &mut [u8], base: u64, range: Range<u64>, flags: u64) -> Result<()> {
    let entries: &mut [u64] = bytemuck::cast_slice_mut(table);
    for address in (range.start..range.end).step_by(PAGE_SIZE) {
        let entry = &mut entries[((address - base) >> 12) as usize];
        if *entry & 1 == 0 {
            *entry = address | flags | 3;
        }
    }
    Ok(())
}

/// Note: This only ensures that the range this L2 table covers is mapped.
fn ensure_l2_table(table: &mut [u8], base: u64, range: Range<u64>, flags: u64) -> Result<()> {
    const SIZE: u64 = 1 << 21;
    let entries: &mut [u64] = bytemuck::cast_slice_mut(table);
    for block in (range.start & !(SIZE - 1)..range.end).step_by(SIZE as usize) {
        let entry = &mut entries[((block - base) >> 21) as usize];
        if *entry & 3 == 1 {
            continue;
        }
        let start = range.start.max(block);
        let end = range.end.min(block + SIZE);
        if *entry & 1 == 0 && start == block && end == block + SIZE {
            *entry = block | flags | 1;
        } else {
            ensure_l3_table(ensure_child_table(entry)?, block, start..end, flags)?;
        }
    }
    Ok(())
}

/// Note: This only ensures that the range this L1 table covers is mapped.
fn ensure_l1_table(table: &mut [u8], base: u64, range: Range<u64>, flags: u64) -> Result<()> {
    const SIZE: u64 = 1 << 30;
    let entries: &mut [u64] = bytemuck::cast_slice_mut(table);
    for block in (range.start & !(SIZE - 1)..range.end).step_by(SIZE as usize) {
        let entry = &mut entries[((block - base) >> 30) as usize];
        if *entry & 3 == 1 {
            continue;
        }
        let start = range.start.max(block);
        let end = range.end.min(block + SIZE);
        if *entry & 1 == 0 && start == block && end == block + SIZE {
            *entry = block | flags | 1;
        } else {
            ensure_l2_table(ensure_child_table(entry)?, block, start..end, flags)?;
        }
    }
    Ok(())
}

/// Ensures that the L0 table has entries for the given range of physical entries.
fn ensure_l0_table(table: &mut [u8], range: Range<u64>, flags: u64) -> Result<()> {
    const SIZE: u64 = 1 << 39;
    let entries: &mut [u64] = bytemuck::cast_slice_mut(table);
    for block in (range.start & !(SIZE - 1)..range.end).step_by(SIZE as usize) {
        let child = ensure_child_table(&mut entries[(block >> 39) as usize])?;
        ensure_l1_table(
            child,
            block,
            range.start.max(block)..range.end.min(block + SIZE),
            flags,
        )?;
    }
    Ok(())
}

/// Map existing Barrelfish allocations at 0xffff000000000000 + physical address.
pub fn alloc_high_page_tables() -> Result<u64> {
    let mmap = boot::memory_map(MemoryType::LOADER_DATA)?;
    let root = allocate_table()?;

    for region in mmap.entries() {
        let end = region.phys_start + region.page_count * PAGE_SIZE as u64;

        if region.ty == MemoryType::MMIO || region.ty == MemoryType::MMIO_PORT_SPACE {
            ensure_l0_table(root, region.phys_start..end, DEVICE_FLAGS)?;
            continue;
        }

        if region.ty != MemoryType::CONVENTIONAL && region.ty.0 < 0x8000_0000 {
            // We don't need UEFI mem in high mem.
            continue;
        }

        ensure_l0_table(root, region.phys_start..end, RAM_FLAGS)?;
    }

    // Fill address-space holes with device mappings without replacing RAM mappings.
    // NOTE: This is bogus. Barrelfish should map MMIO on demand when using them, we cannot parse ACPI tables here...
    ensure_l0_table(root, 0..1 << 48, DEVICE_FLAGS)?;

    Ok(root.as_ptr() as u64)
}

pub struct Stack {
    pub base: u64,
    pub top: u64,
}

pub fn allocate_stack(size: usize) -> Result<Stack> {
    let pages = size.div_ceil(PAGE_SIZE);

    let addr = boot::allocate_pages(AllocateType::AnyPages, MEM_TYPE_STACK, pages)?;
    let addr = addr.as_ptr() as u64;

    Ok(Stack {
        base: addr,
        // 16 below the top is a barrelfish convention.
        top: addr + (pages * PAGE_SIZE) as u64 - 16,
    })
}
