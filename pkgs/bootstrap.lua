local miq = require "miq"

local fetch = miq.fetch
local package = miq.package
local f = miq.f

local bootstrap = {}

bootstrap.bootstrap_tools = fetch {
	url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz",
}

bootstrap.busybox = fetch {
	url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
	executable = true,
}

bootstrap.toybox = fetch {
	url = "http://landley.net/toybox/bin/toybox-x86_64",
	executable = true,
}

bootstrap.unpack_bootstrap_tools = fetch {
	url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
	executable = true,
}

bootstrap.bootstrap = package {
	name = "bootstap",
	version = "1.0",
	deps = {},
	script = f [[
    set -exu
    {{bootstrap.toybox}} mkdir -p $HOME/bin
    export PATH="$HOME/bin:${PATH}"
    {{bootstrap.toybox}} ln -vs {{bootstrap.toybox}} $HOME/bin/ln
    {{bootstrap.toybox}} ln -vs {{bootstrap.toybox}} $HOME/bin/cp
    {{bootstrap.toybox}} ln -vs {{bootstrap.toybox}} $HOME/bin/tar
    {{bootstrap.toybox}} ln -vs {{bootstrap.toybox}} $HOME/bin/mkdir
    {{bootstrap.toybox}} ln -vs {{bootstrap.toybox}} $HOME/bin/chmod

    cp -v {{bootstrap.bootstrap_tools}} $HOME/bootstrap.tar.xz
    mkdir -pv $miq_out
    pushd $miq_out
    tar -xvf $HOME/bootstrap.tar.xz

    export out=$miq_out
    export tarball={{bootstrap.bootstrap_tools}}
    export builder={{bootstrap.busybox}}
    {{bootstrap.unpack_bootstrap_tools}}
  ]],
	env = {},
}

bootstrap.stdenv = function(input)
	if input.env == nil then
		input.env = {
			PATH = f "{{bootstrap.bootstrap}}/bin",
			CC = f "{{bootstrap.bootstrap}}/bin/gcc",
			CFLAGS = "-O2 -pipe -pie -fPIE -fPIC",
		}
	end
	return miq.package(input)
end

return bootstrap
