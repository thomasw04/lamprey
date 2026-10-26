use anyhow::{Result, bail, ensure};
use elf::{
    ElfBytes,
    abi::{EM_AARCH64, R_AARCH64_NONE, R_AARCH64_RELATIVE},
    endian::LittleEndian,
    file::Class,
    relocation::Rela,
};

pub fn valid(elf: &ElfBytes<LittleEndian>) -> Result<()> {
    ensure!(
        elf.ehdr.class == Class::ELF64 && elf.ehdr.e_machine == EM_AARCH64,
        "Invalid ELF file: expected 64-bit AArch64 ELF, got class {:?} and machine {:?}",
        elf.ehdr.class,
        elf.ehdr.e_machine
    );
    Ok(())
}

pub fn do_rela(bytes: &mut [u8], base: u64, poffset: u64, rela: Rela) -> Result<()> {
    if rela.r_type == R_AARCH64_NONE {
        return Ok(());
    }

    if rela.r_type != R_AARCH64_RELATIVE {
        bail!("Unsupported relocation type: {}", rela.r_type);
    }

    if rela.r_sym != 0 {
        bail!("Unsupported relocation symbol: {}", rela.r_sym);
    }

    let offset = (rela.r_offset - base) as usize;
    let value = poffset.wrapping_add_signed(rela.r_addend);
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
