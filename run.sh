#!/usr/bin/env bash
set -euo pipefail

EFI=$(cargo build --bin lamprey --release --message-format=json |
  jq -r '
    select(
      .reason == "compiler-artifact"
      and (.target.kind | index("bin"))
      and .target.name == "lamprey"
      and .executable != null
    )
    | .executable
  ')

echo "EFI: $EFI"

exec qemu-system-aarch64 \
  -machine virt,virtualization=on,gic-version=3 -cpu cortex-a72 -m 512 \
  -bios "$QEMU_EFI" \
  -kernel "$EFI" \
  -initrd initrd.cpio \
  -net none -display none -monitor none -serial stdio
