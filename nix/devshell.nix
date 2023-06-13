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
        rust-bin.nightly.latest.rustfmt
        rust-analyzer-unwrapped
        pkg-config
        diesel-cli
        sqlite-interactive.dev

        graph-easy
        lua5_4
        lua-language-server
        stylua
        graphviz-nox
        bubblewrap
      ];
      NIX_DEBUG = "1";
      RUST_BACKTRACE = "0";
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
