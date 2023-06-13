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
    "${bname}-musl" = {
      target = "${proc}-unknown-linux-musl";
      static = true;
      stdenv = pkgs.pkgsCross.musl64.stdenv;
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
      doCheck = false;
      name = bname;
      inherit src;
      inherit (v) target;
      inherit (cargo-toml.package) version;
      cargoLock = {
        lockFile = src + "/Cargo.lock";
        outputHashes = {
          "mlua-0.9.0-beta.2" = "sha256-DmIBCyhDHuRjn6XL/2PYsaLCfR09davsysN7oq2aD9M=";
        };
      };
      CARGO_BUILD_TARGET = v.target;

      RUSTFLAGS = pkgs.lib.optionalString v.static "-C target-feature=+crt-static";

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
