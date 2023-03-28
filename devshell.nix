{
  pkgs,
  config,
  ...
}: {
  devShells = {
    default = pkgs.mkShell {
      name = "miq-shell";
      packages = [
        config.packages.toolchain
        pkgs.rust-analyzer-unwrapped
        pkgs.pkg-config
        pkgs.diesel-cli
        pkgs.sqlite-interactive.dev
      ];
      NIX_DEBUG = "1";
    };
  };

  packages = {
    toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    # nix build ~/Documents/miq#bootstrap --out-link ~/Documents/miq/devel/nix-bootstrap
    bootstrap = with pkgs;
      buildEnv {
        name = "bootstrap-env";
        paths = [
          gnumake
          # binutils
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
        ];
        pathsToLink = [
          "/bin"
        ];
      };
  };
}
