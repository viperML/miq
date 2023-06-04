local miq = require "miq"
local inspect = miq.inspect
local fetch = miq.fetch
local package = miq.package
local f = miq.f
local trace = miq.trace



local pkgs = {}

pkgs.bootstrap_tools = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
}

pkgs.busybox = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
  executable = true
}

-- pkgs.toybox = fetch {
--     url = "http://landley.net/toybox/bin/toybox-x86_64",
--     executable = true
-- }

pkgs.unpack_bootstrap_tools = fetch {
  url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
  executable = true
}

miq.trace(pkgs)

pkgs.foo = package {
  name = "foo",
  version = "1.0",
  deps = {
  },
  script = f[[
    set -x
    {{pkgs.busybox}} ls
    ls ${HOME}
    exit 1
  ]],
  env = {
    FOO = "bar",
    FOOO = "baar",
  }
}

-- pkgs.bootstrap = package {
--   name = "bootstap",
--   version = "1.0",
--   deps = {
--   },
--   script = f[[
--     set -exu
--     {{pkgs.toybox}} mkdir -p $HOME/bin
--     export PATH="$HOME/bin:${PATH}"
--     {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/ln
--     {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/cp
--     {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/tar
--     {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/mkdir
--     {{pkgs.toybox}} ln -vs {{pkgs.toybox}} $HOME/bin/chmod

--     cp -v {{pkgs.bootstrap_tools}} $HOME/bootstrap.tar.xz
--     mkdir -pv $miq_out
--     pushd $miq_out
--     tar -xvf $HOME/bootstrap.tar.xz

--     export out=$miq_out
--     export tarball={{pkgs.bootstrap_tools}}
--     export builder={{pkgs.busybox}}
--     {{pkgs.unpack_bootstrap_tools}}
--   ]],
--   env = {
--   }
-- }



return pkgs
