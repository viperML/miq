local miq = require "miq"
local utils = require "utils"

local f = miq.f

local x = {}

x.bootstrap_tools = miq.fetch {
	url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz",
}

x.busybox = miq.fetch {
	url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
	executable = true,
}

x.toybox = miq.fetch {
	url = "http://landley.net/toybox/bin/toybox-x86_64",
	executable = true,
}

x.unpack_bootstrap_tools = miq.fetch {
	url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
	executable = true,
}

x.bootstrap = miq.package {
	name = "bootstrap",
	script = f [[
    set -exu
    {{x.toybox}} mkdir -p $HOME/bin
    export PATH="$HOME/bin:${PATH}"
    {{x.toybox}} ln -vs {{x.toybox}} $HOME/bin/ln
    {{x.toybox}} ln -vs {{x.toybox}} $HOME/bin/cp
    {{x.toybox}} ln -vs {{x.toybox}} $HOME/bin/tar
    {{x.toybox}} ln -vs {{x.toybox}} $HOME/bin/mkdir
    {{x.toybox}} ln -vs {{x.toybox}} $HOME/bin/chmod

    export out=$miq_out
    export tarball={{x.bootstrap_tools}}
    export builder={{x.busybox}}
    {{x.unpack_bootstrap_tools}}
  ]],
}

-- x.stdenv = function(input)
-- 	local bootstrap = bootstrap
-- 	input.env = {
-- 		PATH = f "{{x.bootstrap}}/bin",
-- 		CC = f "{{x.bootstrap}}/bin/gcc",
-- 		CFLAGS = "-O2 -pipe -pie -fPIE -fPIC",
-- 	}
-- 	return miq.package(input)
-- end
x.cc = utils.ccBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	cc = x.bootstrap,
}

x.ld = utils.ldBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	ld = x.bootstrap,
}

x.stdenv = utils.stdenvBuilder {
  name = "stage0-stdenv",
	cc = x.cc,
	ld = x.ld,
  shell = x.bootstrap,
  coreutils = x.bootstrap,
	depend = {
		x.bootstrap,
	},
}

x.test = x.stdenv {
	name = "test",
	depend = {},
	script = f [[
    gcc --version
    ls -la
    pwd
    exit 2
  ]],
}

return x
