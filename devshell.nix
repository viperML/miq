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
        pkgs.dhall
        pkgs.dhall-json
        pkgs.dhall-lsp-server
      ];
    };
  };

  packages = {
    toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
    bootstrap = with pkgs;
      buildEnv {
        name = "bootstrap-env";
        paths = [
          gnumake
          binutils
          coreutils
          # mold
          gnutar
          xz
        ];
        pathsToLink = [
          "/bin"
        ];
      };
  };
}
