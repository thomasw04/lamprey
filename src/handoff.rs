use anyhow::Result;
use uefi::boot::{self, PAGE_SIZE};

use crate::{
    mmu::{self, Stack},
    multiboot::Multiboot2,
    types::InlineStr,
};

#[repr(C)]
#[derive(Default)]
pub struct MemRegion {
    pub base: u64,
    pub size: u64,
}

#[repr(C)]
#[derive(Default)]
pub struct CoreData {
    pub boot_magic: u64,
    pub cpu_driver_stack: u64,
    pub cpu_driver_stack_limit: u64,
    pub cpu_driver_globals_pointer: u64,
    pub cpu_driver_entry: u64,
    pub cpu_driver_cmdline: InlineStr<128>,
    pub page_table_root: u64,
    pub memory: MemRegion,
    pub urpc_frame: MemRegion,
    pub monitor_binary: MemRegion,
    pub multiboot_image: MemRegion,
    pub efi_mmap: u64,
    pub start_kernel_ram: u64,
    pub start_free_ram: u64,
    pub chan_id: u32,
    pub kcb: u64,
    pub src_core_id: u8,
    pub dst_core_id: u8,
    pub src_arch_id: u64,
    pub dst_arch_id: u64,
}

// allocate_pages always gives us page-aligned memory.
const _: () = assert!(align_of::<CoreData>() <= PAGE_SIZE);

impl CoreData {
    pub fn allocate() -> Result<&'static mut Self> {
        let pages = size_of::<Self>().div_ceil(PAGE_SIZE);
        let addr =
            boot::allocate_pages(boot::AllocateType::AnyPages, mmu::MEM_TYPE_CORE_DATA, pages)?;

        Ok(unsafe {
            let ptr = addr.cast::<Self>();
            ptr.write(Self::default());
            &mut *ptr.as_ptr()
        })
    }

    pub fn new(mbi: Multiboot2, table_root: u64, stack: Stack) -> Self {
        let mut core_data = Self {
            boot_magic: 0xb001_b000,

            cpu_driver_stack: stack.top,
            cpu_driver_stack_limit: stack.base,

            page_table_root: table_root,

            multiboot_image: MemRegion {
                base: mbi.addr(),
                size: mbi.len() as u64,
            },

            efi_mmap: mbi.mmap(),
            ..Default::default()
        };

        let cpu_driver = mbi.cpu_driver();

        core_data.cpu_driver_entry = cpu_driver.entry;
        core_data.cpu_driver_cmdline = cpu_driver.cmdline;

        core_data
    }
}
