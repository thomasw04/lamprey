{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [ rustup qemu coreutils bashInteractive jq cpio ];
  QEMU_EFI = pkgs.pkgsCross.aarch64-multiplatform.OVMF.firmware;
}
