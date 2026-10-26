# Lamprey

Lamprey is a readable and fast UEFI bootloader for Barrelfish on AArch64.

## Requirements

- Rust and Cargo
- QEMU with AArch64 support
- AArch64 UEFI firmware
- A CPIO archive containing Barrelfish

The provided `nix-shell` supplies the development tools and UEFI firmware.

## Boot archive

Lamprey loads `initrd.cpio`. The archive must contain `hagfish.cfg` at its root
and every file referenced by that configuration.

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

Paths refer to files in the archive and may start with `/`. Remaining arguments
are passed to the component. `stack` specifies the CPU-driver stack size in
bytes and defaults to 1 MiB.

## Run

```sh
nix-shell
./run.sh
```
