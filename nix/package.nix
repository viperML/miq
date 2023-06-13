{
  rustPlatform,
  pname,
  src,
  version,
  pkg-config,
  sqlite,
  lib,
  targetPlatform,
}:
rustPlatform.buildRustPackage {
  doCheck = false;
  inherit pname src version;
  cargoLock = {
    lockFile = src + "/Cargo.lock";
    outputHashes = {
      "mlua-0.9.0-beta.2" = "sha256-DmIBCyhDHuRjn6XL/2PYsaLCfR09davsysN7oq2aD9M=";
    };
  };

  CARGO_BUILD_TARGET = targetPlatform.config;
  target = targetPlatform.config;

  RUSTFLAGS = lib.optionalString (targetPlatform.libc == "musl") "-C target-feature=+crt-static";

  nativeBuildInputs = [
    pkg-config
  ];
  buildInputs = [
    sqlite
  ];
}
