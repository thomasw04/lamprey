use core::mem::ManuallyDrop;

use alloc::vec::Vec;
use anyhow::Result;
use uefi::{
    boot::{self, PAGE_SIZE},
    mem::memory_map::MemoryMap,
};

use crate::{elf::Program, mmu, payload::Module, types::InlineStr};

mod format;

pub struct CpuDriver {
    pub entry: u64,
    pub cmdline: InlineStr<128>,
}

pub struct Builder {
    /// The bytes will be leaked, to ensure the mmap does not change. The kernel can reclaim it later.
    bytes: ManuallyDrop<Vec<u8>>,
    cpu_driver: Option<CpuDriver>,
    entry: Option<u64>,
    stack: usize,
}

impl Builder {
    pub fn new() -> Self {
        const DEFAULT_STACK_SIZE: usize = 1024 * 1024; // 1 MiB

        Self {
            bytes: ManuallyDrop::new(Vec::new()),
            cpu_driver: None,
            entry: None,
            stack: DEFAULT_STACK_SIZE,
        }
    }

    fn tag_header(&mut self, typ: u32, size: u32) -> Result<()> {
        let tag = format::Tag::new(typ, size);
        let tag_bytes = bytemuck::bytes_of(&tag);
        self.bytes.try_reserve(tag_bytes.len())?;
        self.bytes.extend_from_slice(tag_bytes);
        Ok(())
    }

    fn tag_header_len(size: u32) -> u32 {
        size + 8
    }

    /// Write the memory map tag to the destination buffer.
    ///
    /// This will panic if the destination buffer is too small to hold the memory map tag.
    fn mmap<'a>(dst: &'a mut [u8], mmap: &impl MemoryMap) -> &'a mut [u8] {
        let bytes = mmap.buffer();

        if dst.len() < Self::mmap_len(mmap) as usize {
            panic!("Destination buffer is too small for memory map");
        }

        let header = format::UefiMmap::new(
            mmap.meta().desc_size as u32,
            mmap.meta().desc_version,
            bytes.len() as u32,
        );

        let dst = format::push_bytes(dst, bytemuck::bytes_of(&header));
        let dst = format::push_bytes(dst, bytes);
        format::push_padding(dst, 3)
    }

    fn mmap_offset(&self) -> u32 {
        // TAGS (bytes.len()) + HEADER (8 bytes)
        self.bytes.len() as u32 + 8
    }

    fn mmap_len(mmap: &impl MemoryMap) -> u32 {
        Self::tag_header_len(mmap.buffer().len() as u32 + 16)
    }

    pub fn tag(&mut self, typ: u32, payload: &[u8]) -> Result<()> {
        self.tag_header(typ, payload.len() as u32)?;

        self.bytes.try_reserve(payload.len())?;
        self.bytes.extend_from_slice(payload);

        // align up to 8 bytes
        let padding = (8 - (payload.len() % 8)) % 8;
        self.bytes.try_reserve(padding)?;
        for _ in 0..padding {
            self.bytes.push(0);
        }
        Ok(())
    }

    pub fn module(&mut self, module: &Module) -> Result<()> {
        let cmd_len = module.cmdline().map(|c| c.len()).unwrap_or(0);

        let header = format::Module::new(
            module.addr(),
            module.addr() + module.size() - 1,
            cmd_len as u32,
        );

        let header = bytemuck::bytes_of(&header);

        self.bytes.try_reserve(header.len())?;
        self.bytes.extend_from_slice(header);

        self.bytes.try_reserve(cmd_len + 1)?;

        if let Some(cmdline) = module.cmdline() {
            self.bytes.extend_from_slice(cmdline.as_bytes());
        }

        self.bytes.push(0); // null terminate the cmdline

        // align up to 8 bytes
        let padding = (8 - (self.bytes.len() % 8)) % 8;
        self.bytes.try_reserve(padding)?;
        for _ in 0..padding {
            self.bytes.push(0);
        }

        Ok(())
    }

    pub fn cpu_driver(&mut self, module: &Module, prog: &Program) -> Result<()> {
        self.module(module)?;

        let Some(cmdline) = module.cmdline() else {
            return Err(anyhow::anyhow!("CPU driver module must have a cmdline"));
        };

        self.cpu_driver = Some(CpuDriver {
            entry: prog.entry,
            cmdline: InlineStr::new(cmdline),
        });

        Ok(())
    }

    pub fn boot_driver(&mut self, module: &Module, prog: &Program) -> Result<()> {
        self.module(module)?;
        self.entry = Some(prog.entry);
        Ok(())
    }

    pub fn set_stack_size(&mut self, size: usize) {
        self.stack = size;
    }

    pub fn entry(&self) -> u64 {
        self.entry
            .expect("Boot driver module must be set before preparing the handoff")
    }

    pub fn stack(&self) -> usize {
        self.stack
    }

    /// Exit boot services and build the multiboot2 structure.
    ///
    /// After this function alloc/drop is illegal! Also any reference to uefi boot services must be released beforehand!
    pub unsafe fn exit_and_build(self, mmap: impl MemoryMap) -> Multiboot2 {
        // Ensure that the mmap is dropped before we exit boot services.
        let (addr, pages) = {
            let estimated = self.mmap_offset() + Self::mmap_len(&mmap);
            let pages = estimated.div_ceil(PAGE_SIZE as u32) + 1; // +1 for the end tag and new mmap entry.
            drop(mmap);

            (
                boot::allocate_pages(
                    boot::AllocateType::AnyPages,
                    mmu::MEM_TYPE_MULTIBOOT,
                    pages as usize,
                )
                .expect("Failed to allocate pages for multiboot2 structure"),
                pages,
            )
        };

        // Allocation/Deallocation is illegal now.
        let (mmap, dst) = unsafe {
            (
                boot::exit_boot_services(None),
                core::slice::from_raw_parts_mut(addr.as_ptr(), pages as usize * PAGE_SIZE),
            )
        };

        let size = self.mmap_offset() + Self::mmap_len(&mmap);

        // Write the multiboot2 header and the tags to the allocated memory.
        let header = format::Header::new(size);

        let dst = format::push_bytes(dst, bytemuck::bytes_of(&header));
        let dst = format::push_bytes(dst, &self.bytes);

        // Write the memory map tag to the allocated memory.
        let dst = Self::mmap(dst, &mmap);

        // Write the end tag to the allocated memory.
        let end_tag = format::Tag::new(0, 0);
        let _ = format::push_bytes(dst, bytemuck::bytes_of(&end_tag));

        let mmap_offset = self.mmap_offset();

        // Earlier might make more sense. But tbh it doesn't matter, we fail anyways and this avoids borrowchk issues.
        let Some(cpu) = self.cpu_driver else {
            panic!("CPU driver module must be set before exiting boot services");
        };

        Multiboot2::new(addr.as_ptr() as u64, mmap_offset, size, cpu)
    }
}

pub struct Multiboot2 {
    addr: u64,
    mmap_offset: u32,
    len: u32,

    cpu_driver: CpuDriver,
}

impl Multiboot2 {
    fn new(addr: u64, mmap_offset: u32, len: u32, cpu: CpuDriver) -> Self {
        Multiboot2 {
            addr,
            mmap_offset,
            len,
            cpu_driver: cpu,
        }
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn mmap(&self) -> u64 {
        self.addr() + self.mmap_offset as u64
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn cpu_driver(self) -> CpuDriver {
        self.cpu_driver
    }
}
