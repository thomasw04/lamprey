{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  packages = with pkgs; [ rustup qemu coreutils bashInteractive jq cpio ncurses ];
  QEMU_EFI = pkgs.pkgsCross.aarch64-multiplatform.OVMF.firmware;
}
