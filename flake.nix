{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-filter.url = "github:numtide/nix-filter";
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      perSystem = {
        system,
        pkgs,
        ...
      }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [
            inputs.rust-overlay.overlays.default
          ];
        };

        _module.args.src = inputs.nix-filter.lib {
          root = inputs.self;
          include = [
            (inputs.nix-filter.lib.inDirectory "src")
            "Cargo.toml"
            "Cargo.lock"
            "build.rs"
            "migrations"
          ];
        };

        legacyPackages = pkgs;

        imports = [
          ./devshell.nix
          ./packages.nix
          ./trivial.nix
        ];
      };
    };
}
