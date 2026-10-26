use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Header {
    total_size: u32,
    reserved: u32,
}

impl Header {
    pub fn new(total_size: u32) -> Self {
        Self {
            total_size,
            reserved: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Tag {
    typ: u32,
    size: u32,
}

impl Tag {
    pub fn new(typ: u32, size: u32) -> Self {
        Self {
            typ,
            // Barrelfish advances by tag.size without rounding it up as Multiboot2 specifies.
            size: (size + 8).next_multiple_of(8),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct UefiMmap {
    header: Tag,
    desc_size: u32,
    desc_version: u32,
}

impl UefiMmap {
    pub fn new(desc_size: u32, desc_version: u32, size: u32) -> Self {
        Self {
            header: Tag {
                typ: 17,
                size: size + 16,
            },
            desc_size,
            desc_version,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Module {
    header: Tag,
    begin: u64,
    end: u64,
}

impl Module {
    pub fn new(begin: u64, end: u64, size: u32) -> Self {
        Self {
            // START (8) + END (8) + CMDLINE (size) + NUL (1)
            header: Tag::new(19, 8 + 8 + size + 1),
            begin,
            end,
        }
    }
}

pub fn push_bytes<'a>(dst: &'a mut [u8], bytes: &[u8]) -> &'a mut [u8] {
    if dst.len() < bytes.len() {
        // This should work even after we exited boot-services. So no anyhow allocation.
        panic!("Not enough space in destination buffer. This is a bug!");
    }

    dst[..bytes.len()].copy_from_slice(bytes);
    &mut dst[bytes.len()..]
}

/// Computes how much padding is needed to align `size` to 2^`pow`.
pub fn align_addend(size: usize, pow: usize) -> usize {
    let mask = (1usize << pow) - 1;
    size.wrapping_neg() & mask
}

pub fn push_padding(dst: &mut [u8], pow: usize) -> &mut [u8] {
    let addend = align_addend(dst.as_ptr() as usize, pow);
    if dst.len() < addend {
        // This should work even after we exited boot-services. So no anyhow allocation.
        panic!("Not enough space in destination buffer. This is a bug!");
    }
    dst[..addend].fill(0);
    &mut dst[addend..]
}
