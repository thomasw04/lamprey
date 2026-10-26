use core::slice;

use anyhow::Result;
use uefi::table::cfg::ConfigTableEntry;

use crate::multiboot::Builder;

const MULTIBOOT_TAG_TYPE_ACPI_OLD: u32 = 14;
const MULTIBOOT_TAG_TYPE_ACPI_NEW: u32 = 15;
const ACPI_1_RSDP_SIZE: usize = 20;
const ACPI_2_RSDP_SIZE: usize = 36;

pub fn emit(mbi: &mut Builder) -> Result<()> {
    uefi::system::with_config_table(|tables| {
        for table in tables {
            let (tag_type, size) = match table.guid {
                ConfigTableEntry::ACPI_GUID => {
                    (MULTIBOOT_TAG_TYPE_ACPI_OLD, ACPI_1_RSDP_SIZE)
                }
                ConfigTableEntry::ACPI2_GUID => {
                    (MULTIBOOT_TAG_TYPE_ACPI_NEW, ACPI_2_RSDP_SIZE)
                }
                _ => continue,
            };

            // UEFI guarantees that an ACPI configuration-table entry points to an RSDP.
            let rsdp = unsafe { slice::from_raw_parts(table.address.cast(), size) };
            mbi.tag(tag_type, rsdp)?;
        }

        Ok(())
    })
}
