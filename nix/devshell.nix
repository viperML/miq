{
  pkgs,
  config,
  ...
}: {
  devShells.default = let
    toolchain' = pkgs.miq-toolchain.override {
      extensions = [
        "rust-src"
      ];
    };
  in
    pkgs.mkShell {
      name = "miq-shell";
      RUST_SRC_PATH = "${toolchain'}/lib/rustlib/src/rust/library";
      packages = with pkgs; [
        toolchain'
        pkg-config
        diesel-cli
        sqlite-interactive.dev
        bubblewrap

        rust-analyzer-unwrapped
        rust-bin.nightly.latest.rustfmt

        graph-easy
        lua5_4
        lua-language-server
        stylua
        graphviz-nox
      ];
      NIX_DEBUG = "1";
      RUST_BACKTRACE = "0";
    };

  devShells.nightly = pkgs.mkShell {
    name = "nightly";
    packages = with pkgs; [
      (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default))
      pkg-config
      diesel-cli
      sqlite-interactive.dev
      cargo-udeps
    ];
  };

  packages = {
    # toolchain' = with config.packages;
    #   pkgs.symlinkJoin {
    #     inherit (toolchain) name;
    #     paths = [toolchain];
    #     postBuild = "rm $out/bin/rustfmt";
    #   };
    # toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    # nix build ~/Documents/miq#bootstrap --out-link ~/Documents/miq/devel/nix-bootstrap
    # bootstrap = with pkgs;
    #   buildEnv {
    #     name = "bootstrap-env";
    #     paths = [
    #       gnumake
    #       # binutils
    #       gcc-unwrapped
    #       gcc-unwrapped.lib
    #       bintools-unwrapped
    #       coreutils
    #       gnused
    #       bash
    #       gnugrep
    #       gawk
    #       patchelf

    #       gnutar
    #       xz
    #       gzip
    #       (busybox.override {enableAppletSymlinks = false;})
    #     ];
    #     pathsToLink = [
    #       "/bin"
    #       # "/lib"
    #       # "/include"
    #     ];
    #   };
  };
}
