use anyhow::{Result, bail};
use elf::{
    ElfBytes,
    abi::{PT_LOAD, SHT_RELA},
    endian::LittleEndian,
};
use uefi::boot::{self, MemoryType, PAGE_SIZE};

mod arch;

pub struct Program {
    pub entry: u64,
}

fn allocate<'a>(elf: &ElfBytes<LittleEndian>, typ: MemoryType) -> Result<(&'a mut [u8], u64, u64)> {
    let Some(segs) = elf.segments() else {
        bail!("ELF has no segments");
    };

    let mut align = PAGE_SIZE as u64;
    let mut base = u64::MAX;
    let mut end = 0;

    for seg in segs.iter() {
        if seg.p_type != elf::abi::PT_LOAD {
            continue;
        }

        if seg.p_memsz == 0 {
            continue;
        }

        if seg.p_align > 1 {
            align = align.max(seg.p_align);
        }

        base = base.min(seg.p_vaddr);
        end = end.max(seg.p_vaddr + seg.p_memsz);
    }

    let size = end - base;
    let pages = (size + align - 1).div_ceil(PAGE_SIZE as u64);

    let addr = boot::allocate_pages(boot::AllocateType::AnyPages, typ, pages as usize)?;

    let padding = base.wrapping_sub(addr.as_ptr() as u64) & (align - 1);

    let area = unsafe {
        core::slice::from_raw_parts_mut(addr.as_ptr().add(padding as usize), size as usize)
    };

    Ok((area, base, align))
}

fn copy_into(elf: &ElfBytes<LittleEndian>, area: &mut [u8], base: u64) -> Result<()> {
    let Some(segs) = elf.segments() else {
        bail!("ELF has no segments");
    };

    for seg in segs.iter() {
        if seg.p_type == PT_LOAD && seg.p_memsz > 0 {
            let data = elf.segment_data(&seg)?;
            let offset = (seg.p_vaddr - base) as usize;
            area[offset..offset + data.len()].copy_from_slice(data);
            let memsz = seg.p_memsz as usize;
            area[offset + data.len()..offset + memsz].fill(0);
        }
    }

    Ok(())
}

fn relocate(elf: &ElfBytes<LittleEndian>, area: &mut [u8], base: u64, voffset: u64) -> Result<()> {
    let Some(secs) = elf.section_headers() else {
        // No section headers, nothing to relocate.
        return Ok(());
    };

    let mut poffset = (area.as_ptr() as u64).wrapping_sub(base);
    poffset = poffset.wrapping_add(voffset);

    for sec in secs.iter() {
        if sec.sh_type != SHT_RELA {
            continue;
        }

        for rela in elf.section_data_as_relas(&sec)? {
            arch::do_rela(area, base, poffset, rela)?;
        }
    }
    Ok(())
}

pub fn load(bytes: &[u8], voffset: u64, mem_typ: MemoryType) -> Result<Program> {
    let elf = ElfBytes::<LittleEndian>::minimal_parse(bytes)?;
    arch::valid(&elf)?;

    let (area, base, _) = allocate(&elf, mem_typ)?;
    copy_into(&elf, area, base)?;
    relocate(&elf, area, base, voffset)?;
    crate::asm::sync_instruction_cache(area);

    let entry = elf.ehdr.e_entry - base + area.as_ptr() as u64 + voffset;

    Ok(Program { entry })
}
