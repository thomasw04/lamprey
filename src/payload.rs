use core::ptr::NonNull;

use alloc::vec::Vec;
use anyhow::{Context, Result, bail};
use hashbrown::HashMap;
use uefi::{boot, cstr16, fs::FileSystem};

use crate::{config::Component, mmu};

pub type Index<'a> = HashMap<&'a str, &'a [u8]>;

pub struct Module<'a> {
    addr: u64,
    size: u64,
    cmdline: Option<&'a str>,
}

impl Module<'_> {
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: A module may only be created by `to_mem`, which ensures that the address and size are valid.
        unsafe { core::slice::from_raw_parts(self.addr as *const u8, self.size as usize) }
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn cmdline(&self) -> Option<&str> {
        self.cmdline
    }
}

pub fn load() -> Result<Vec<u8>> {
    let volume = boot::get_image_file_system(boot::image_handle())?;
    let mut fs = FileSystem::new(volume);

    fs.read(cstr16!("\\initrd"))
        .or(fs.read(cstr16!("\\lamprey\\boot.cpio")))
        .context("failed to load boot modules")
}

pub fn build_index(data: &[u8]) -> Result<Index<'_>> {
    let mut index = HashMap::with_capacity(64);

    for entry in cpio_reader::iter_files(data) {
        index.try_reserve(1)?;
        index.insert(entry.name(), entry.file());
    }

    Ok(index)
}

fn emplace(data: &[u8]) -> Result<NonNull<u8>> {
    const PAGE_SIZE: usize = 4096;

    let pages = data.len().div_ceil(PAGE_SIZE);
    let addr = boot::allocate_pages(boot::AllocateType::AnyPages, mmu::MEM_TYPE_ELF, pages)?;

    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), addr.as_ptr(), data.len());
    }

    Ok(addr)
}

pub fn to_mem<'a>(comp: &Component<'a>, index: &Index) -> Result<Module<'a>> {
    let path = comp.path.strip_prefix('/').unwrap_or(comp.path);

    let Some(file) = index.get(path) else {
        bail!("file {} not found.", comp.path)
    };

    if file.is_empty() {
        bail!("file {} is empty", comp.path);
    }

    let addr = emplace(file)?;

    Ok(Module {
        addr: addr.as_ptr() as u64,
        size: file.len() as u64,
        cmdline: Some(comp.cmdline),
    })
}
