local miq = require("miq")

local fetch = miq.fetch
local package = miq.package
local f = miq.f

local pkgs = {}

pkgs.bootstrap_tools = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
}

pkgs.busybox = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
  executable = true
}

pkgs.toybox = fetch {
    url = "http://landley.net/toybox/bin/toybox-x86_64",
    executable = true
}

pkgs.unpack_bootstrap_tools = fetch {
  url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
  executable = true
}

pkgs.bootstrap = package {
  name = "bootstap",
  version = "1.0",
  deps = {
  },
  script = f[[
    set -exu
    {{pkgs.toybox}} mkdir -p $HOME/bin
    export PATH="$HOME/bin:${PATH}"
    {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/ln
    {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/cp
    {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/tar
    {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/mkdir
    {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/chmod

    cp -v {{pkgs.bootstrap_tools}} $HOME/bootstrap.tar.xz
    mkdir -pv $miq_out
    pushd $miq_out
    tar -xvf $HOME/bootstrap.tar.xz

    export out=$miq_out
    export tarball={{pkgs.bootstrap_tools}}
    export builder={{pkgs.busybox}}
    {{pkgs.unpack_bootstrap_tools}}
  ]],
  env = {
  }
}

pkgs.stdenv = function(input)
  if input.env == nil then
    input.env = {}
  end

  input.env = {
    PATH = f"{{pkgs.bootstrap}}/bin",
    CC = f"{{pkgs.bootstrap}}/bin/gcc",
    CFLAGS = "-O2 -pipe -pie -fPIE -fPIC"
  }

  return miq.package(input)
end

local c_example = [[
#include <stdio.h>
#include <stdlib.h>

int main() {
  printf("Hello World");
  exit(0);
}
]]

pkgs.test_bootstrap = pkgs.stdenv {
  name = "test_bootstrap",
  script = f[[
tee main.c <<EOF
{{c_example}}
EOF

$CC main.c
]]
}


return pkgs
