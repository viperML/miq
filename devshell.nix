{
  pkgs,
  config,
  ...
}: {
  devShells = {
    default = let
      venv = pkgs.buildEnv {
        name = "python-venv";
        paths = [
          (pkgs.python311.withPackages (p: [
            p.setuptools
            p.build
            p.click
            p.toml
          ]))

          pkgs.black
          pkgs.ruff
        ];
      };
    in
      pkgs.mkShell {
        name = "miq-shell";
        packages = [
          config.packages.toolchain'
          pkgs.rust-bin.nightly.latest.rustfmt
          pkgs.rust-analyzer-unwrapped
          pkgs.pkg-config
          pkgs.diesel-cli
          pkgs.sqlite-interactive.dev
          pkgs.graph-easy
          pkgs.lua5_4

          venv
        ];
        NIX_DEBUG = "1";
        RUST_BACKTRACE = "full";
        shellHook = ''
          ln -Tsf ${venv} .venv
        '';
      };
  };

  packages = {
    toolchain' = with config.packages;
      pkgs.symlinkJoin {
        inherit (toolchain) name;
        paths = [toolchain];
        postBuild = "rm $out/bin/rustfmt";
      };
    toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    # nix build ~/Documents/miq#bootstrap --out-link ~/Documents/miq/devel/nix-bootstrap
    bootstrap = with pkgs;
      buildEnv {
        name = "bootstrap-env";
        paths = [
          gnumake
          # binutils
          gcc-unwrapped
          gcc-unwrapped.lib
          bintools-unwrapped
          coreutils
          gnused
          bash
          gnugrep
          gawk
          patchelf

          gnutar
          xz
          gzip
          (busybox.override {enableAppletSymlinks = false;})
        ];
        pathsToLink = [
          "/bin"
          # "/lib"
          # "/include"
        ];
      };
  };
}
