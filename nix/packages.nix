{
  inputs,
  config,
  ...
}: {
  flake.overlays.default = final: prev: let
    src = inputs.nix-filter.lib {
      root = inputs.self;
      include = [
        (inputs.nix-filter.lib.inDirectory "src")
        "Cargo.toml"
        "Cargo.lock"
        "build.rs"
        "migrations"
      ];
    };
    cargo-toml = builtins.fromTOML (builtins.readFile (src + "/Cargo.toml"));
  in {
    miq-toolchain = final.pkgsBuildHost.rust-bin.stable.latest.default.override {
      extensions = [];
      targets = [final.targetPlatform.config];
    };

    miq = final.callPackage ./package.nix {
      inherit src;
      pname = cargo-toml.package.name;
      version = cargo-toml.package.version;
      rustPlatform = final.makeRustPlatform {
        cargo = final.miq-toolchain;
        rustc = final.miq-toolchain;
      };
    };
  };

  perSystem = {
    pkgs,
    system,
    ...
  }: {
    _module.args.pkgs = import inputs.nixpkgs {
      inherit system;
      overlays = [
        inputs.rust-overlay.overlays.default
        config.flake.overlays.default
      ];
    };

    legacyPackages = pkgs;

    checks = {inherit (pkgs) miq;};

    packages = {
      default = pkgs.miq;

      inherit
        (pkgs)
        miq
        miq-toolchain
        ;

      miq-static = pkgs.pkgsCross.musl64.miq;
    };
  };
}
