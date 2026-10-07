{
  # bise on NixOS (and Nix on any Linux): the prebuilt release, patched
  # for the Nix store (nix/package.nix).
  #
  #   nix profile install github:gvergnaud/bise     then: bise
  #   nix run github:gvergnaud/bise
  #   nix profile upgrade bise                       (a new release)
  #
  # nix/sources.json pins the release (URLs + sha256): the release flow
  # commits it once a release is published (packaging/publish-release.sh
  # --publish, from the release's nix-sources.json), so main's HEAD
  # installs the latest release and a tag's own commit carries the one
  # before it.
  description = "bise: one terminal, many agents";

  # a stable NixOS branch, pinned by flake.lock; `--override-input nixpkgs
  # nixpkgs` (or inputs.bise.inputs.nixpkgs.follows) uses your own
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      packages = forAll (pkgs: rec {
        bise = pkgs.callPackage ./nix/package.nix { };
        default = bise;
      });
      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.bise}/bin/bise";
        };
      });
      overlays.default = final: _prev: { bise = final.callPackage ./nix/package.nix { }; };
    };
}
