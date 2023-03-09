{
  pkgs,
  config,
  ...
}: {
  devShells.default = pkgs.mkShell {
    name = "miq-shell";
    packages = [
      config.packages.toolchain
      pkgs.rust-analyzer-unwrapped
    ];
  };

  packages.toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
}
