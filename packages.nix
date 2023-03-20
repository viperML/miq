# {
#   self,
#   config,
#   ...
# }: let
# in {
#   flake.overlayFunc = pkgs: {
#     miq = pkgs.rustPlatform.buildRustPackage {
#     };
#   };
#   perSystem = {pkgs, ...}: {
#     packages = {
#       inherit
#         (config.flake.overlayFunc pkgs)
#         miq
#         ;
#     };
#   };
# }
{
  pkgs,
  config,
  src,
  ...
}: let
  toolchainFor = target:
    pkgs.rust-bin.stable.latest.default.override {
      extensions = [];
      targets = [target];
    };
  cargo-toml = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));
  bname = cargo-toml.package.name;
  proc = pkgs.targetPlatform.uname.processor;
  matrix = {
    "${bname}" = {
      target = "${proc}-unknown-linux-gnu";
      static = false;
      stdenv = pkgs.stdenv;
    };
    "${bname}-static" = {
      target = "${proc}-unknown-linux-musl";
      static = true;
      stdenv = pkgs.pkgsStatic.stdenv;
    };
    "${bname}-clang" = {
      target = "${proc}-unknown-linux-gnu";
      static = false;
      stdenv = pkgs.clangStdenv;
    };
  };
in {
  packages = builtins.mapAttrs (name: v: let
    toolchain = toolchainFor v.target;
    rustPlatform = pkgs.makeRustPlatform {
      cargo = toolchain;
      rustc = toolchain;
      inherit (v) stdenv;
    };
  in
    rustPlatform.buildRustPackage {
      name = bname;
      inherit src;
      inherit (v) target;
      inherit (cargo-toml.package) version;
      cargoLock.lockFile = src + "/Cargo.lock";
      CARGO_BUILD_TARGET = v.target;
      CARGO_BUILD_RUSTFLAGS =
        if v.static
        then "-C target-feature=+crt-static"
        else "";

      nativeBuildInputs = [
        pkgs.pkg-config
      ];
      buildInputs = [
        pkgs.sqlite
      ];
    })
  matrix;

  checks = {
    inherit
      (config.packages)
      miq
      miq-clang
      ;
  };
}
