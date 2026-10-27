# Lamprey

Lamprey is a readable and (hopefully) fast UEFI bootloader for Barrelfish on AArch64 QEMU.

## Why Lamprey instead of Hagfish?

- **Easier to build:** Lamprey has a much simpler build process and requires less setup than Hagfish.
- **Simpler boot setup:** Hagfish's filesystem boot path relies on a disk image and the UEFI shell. Lamprey supports QEMU's `-kernel` and `-initrd` options, avoiding the shell entirely. Creating a CPIO archive is also much easier than setting up a bootable disk image.
- **Faster booting:** Skipping the UEFI shell and disk-based boot process significantly reduces startup time. On my setup, boot times went from around 11 seconds with Hagfish to 0.5 seconds with Lamprey, although I haven't properly benchmarked this.
- **Clean terminal:** The UEFI shell can interfere with terminal input and output. Lamprey just boots your kernel without getting in the way.
- **Other improvements:** Various bug fixes and smaller improvements over Hagfish.

Lamprey aims to make building, booting, and testing Barrelfish kernels on QEMU **simpler, faster, and less intrusive.**

## Requirements

- Rust and Cargo
- QEMU with AArch64 support
- AArch64 UEFI firmware
- A CPIO archive containing Barrelfish

The provided `nix-shell` supplies the development tools and UEFI firmware.

## Boot archive

Lamprey loads `initrd.cpio`. The archive must contain `hagfish.cfg` at its root and every file referenced by that configuration.

To create the archive yourself, place its contents under `initrd/` and run:

```sh
./mkcpio.sh
```

## Configuration

`hagfish.cfg` supports these directives:

```text
# comment
bootdriver /path/to/boot-driver [arguments...]
cpudriver  /path/to/cpu-driver  [arguments...]
module     /path/to/module      [arguments...]
stack 1048576
```

Paths refer to files in the archive and may start with `/`. The path and any following arguments form the component's command line. `stack` specifies the initial boot-driver stack size in bytes and defaults to 1 MiB.

## Run

```sh
nix-shell
./run.sh
```
