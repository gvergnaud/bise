# bise for Nix (NixOS, or Nix on any Linux): the release tarball of
# GitHub Releases (packaging/build-dist.sh, built on Ubuntu 22.04 for
# glibc) made to run without an FHS: autoPatchelfHook points every binary
# at the store's glibc and libgcc, and OpenSSL 3 goes into their RUNPATH,
# because the Bend runtime dlopens libssl.so.3 at its first HTTPS request
# (bend/vendor/http/effs/wire.c).
#
# The app root is $out/lib/bise (VERSION inside: bise finds its own
# binaries next to its resolved exe); $out/bin/bise links to it. The store
# is read-only: `bise update` says to run `nix profile upgrade bise`, and
# a hub moves to the new store path when the upgraded bise is launched in
# its folder (bise_home::release::RootKind::Nix).
#
# sources: nix/sources.json, written from the release's nix-sources.json
# (packaging/make-release.sh) when a release is published. `tarball`
# overrides it (tests: a local tarball).
{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  openssl,
  sources ? lib.importJSON ./sources.json,
  tarball ? null,
}:

let
  system = stdenv.hostPlatform.system;
  pinned =
    sources.${system}
      or (throw "bise: release ${sources.version} has no build for ${system} (bise ships x86_64-linux and aarch64-linux)");
in
stdenv.mkDerivation {
  pname = "bise";
  version = sources.version;

  src = if tarball != null then tarball else fetchurl { inherit (pinned) url sha256; };

  nativeBuildInputs = [ autoPatchelfHook ];
  # libgcc_s (bise, bend-jsrt); glibc comes with stdenv
  buildInputs = [ stdenv.cc.cc.lib ];
  # dlopen()ed, not linked: autoPatchelfHook adds <dep>/lib to every
  # RUNPATH; getLib: openssl's first output is bin, libssl.so.3 is in out
  runtimeDependencies = [ (lib.getLib openssl) ];

  dontConfigure = true;
  dontBuild = true;
  # V8 and the Bend binaries ship as built (packaging.md: never stripped)
  dontStrip = true;

  installPhase = ''
    runHook preInstall
    mkdir -p $out/lib $out/bin
    cp -a app $out/lib/bise
    ln -s ../lib/bise/bise $out/bin/bise
    runHook postInstall
  '';

  doInstallCheck = true;
  installCheckPhase = ''
    HOME=$TMPDIR $out/bin/bise --version
  '';

  meta = {
    description = "bise: one terminal, many agents (prebuilt release)";
    homepage = "https://bise.dev";
    license = lib.licenses.asl20;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = [ "x86_64-linux" "aarch64-linux" ];
    mainProgram = "bise";
  };
}
