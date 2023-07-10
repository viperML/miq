{
  rustPlatform,
  pname,
  src,
  version,
  pkg-config,
  sqlite,
  lib,
  targetPlatform,
  pkgsStatic,
}:
rustPlatform.buildRustPackage {
  doCheck = false;
  inherit pname src version;
  cargoLock.lockFile = src + "/Cargo.lock";

  CARGO_BUILD_TARGET = targetPlatform.config;
  target = targetPlatform.config;

  RUSTFLAGS = lib.optionalString (targetPlatform.libc == "musl") "-C target-feature=+crt-static";

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    sqlite
  ];

  preConfigure = ''
    mkdir -p vendor
    ln -vs ${pkgsStatic.bash}/bin/bash vendor/bash
    ln -vs ${pkgsStatic.busybox}/bin/busybox vendor/busybox
  '';
}
