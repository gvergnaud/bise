# bise without flakes: nix-env -if https://github.com/gvergnaud/bise/archive/main.tar.gz
# (flake.nix and nix/package.nix say the rest)
{
  pkgs ? import <nixpkgs> { },
}:
pkgs.callPackage ./nix/package.nix { }
