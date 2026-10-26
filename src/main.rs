#![no_main]
#![no_std]

use core::arch::asm;

use anyhow::{Result, bail};
use uefi::boot::MemoryType;
use uefi::prelude::*;

use crate::config::Directive;

extern crate alloc;

mod acpi;
mod asm;
mod config;
mod elf;
mod handoff;
mod mmu;
mod multiboot;
mod payload;
mod types;

const MULTIBOOT2_BOOTLOADER_MAGIC: u64 = 0x36d7_6289;

fn exec(dir: &Directive<'_>, index: &payload::Index, mbi: &mut multiboot::Builder) -> Result<()> {
    match dir {
        Directive::BootDriver(comp) => {
            let module = payload::to_mem(comp, index)?;
            let prog = elf::load(module.as_slice(), 0, MemoryType::LOADER_CODE)?;
            mbi.boot_driver(&module, &prog)?;
        }
        Directive::CpuDriver(comp) => {
            let module = payload::to_mem(comp, index)?;
            let prog = elf::load(
                module.as_slice(),
                0xffff_0000_0000_0000,
                mmu::MEM_TYPE_DRIVER,
            )?;
            mbi.cpu_driver(&module, &prog)?;
        }
        Directive::Module(comp) => {
            let module = payload::to_mem(comp, index)?;
            mbi.module(&module)?;
        }
        Directive::Stack(size) => {
            mbi.set_stack_size(*size);
        }
        _ => {}
    }

    Ok(())
}

fn load_and_prepare_kernel() -> Result<multiboot::Builder> {
    let archive = payload::load()?;
    let index = payload::build_index(&archive)?;

    // Lamprey accepts hagfish's config for compat.
    let Some(cfg) = index.get("hagfish.cfg") else {
        bail!("/hagfish.cfg not found");
    };

    let cfg = core::str::from_utf8(cfg)?;
    let mut mbi = multiboot::Builder::new();
    acpi::emit(&mut mbi)?;

    config::parse::for_each(cfg, |dir| exec(dir, &index, &mut mbi))?;
    Ok(mbi)
}

fn prepare_handoff(mbi: multiboot::Builder) -> (u64, u64, u64) {
    let Ok(mmap) = boot::memory_map(MemoryType::LOADER_DATA) else {
        panic!("Failed to allocate memory map");
    };

    let Ok(stack) = mmu::allocate_stack(mbi.stack()) else {
        panic!("Failed to allocate kernel stack");
    };

    let Ok(core_data) = handoff::CoreData::allocate() else {
        panic!("Failed to allocate core data page");
    };

    let Ok(table_root) = mmu::alloc_high_page_tables() else {
        panic!("Failed to allocate page tables");
    };

    let entry = mbi.entry();
    let sp = stack.top;

    // From this point on, alloc/drop is illegal and no UEFI protocol references may be alive.
    // Note: This also means no anyhow!
    let mbi = unsafe { mbi.exit_and_build(mmap) };
    *core_data = handoff::CoreData::new(mbi, table_root, stack);

    (entry, sp, core_data as *const _ as u64)
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let Ok(mbi) = load_and_prepare_kernel() else {
        log::error!("Failed to load and prepare kernel");
        return Status::ABORTED;
    };

    let (entry, sp, core_data) = prepare_handoff(mbi);

    unsafe {
        asm!(
            "mov sp, {stack}",
            "br {entry}",
            stack = in(reg) sp,
            entry = in(reg) entry,
            in("x0") MULTIBOOT2_BOOTLOADER_MAGIC,
            in("x1") core_data,
            options(noreturn),
        )
    }
}
