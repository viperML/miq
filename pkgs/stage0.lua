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
	coreutils = x.bootstrap,
	extra = f [[
    export CFLAGS="\$CFLAGS \
    -idirafter {{x.bootstrap}}/include-libc \
    -isystem {{x.bootstrap}}/include-libc"
  ]],
	-- extra = ""
}

x.test = x.stdenv {
	name = "test",
	depend = {},
	script = f [[
    gcc --version > $miq_out/result
  ]],
}

x.fetchTar = utils.fetchTarBuilder {
	PATH = f "{{x.bootstrap}}/bin",
}

x.cpp_test = x.stdenv {
	name = "cpp_test",
	script = f [[
    tee main.cpp <<EOF
    #include <iostream>
    int
    main()
    {
      std::cout << "Hello world!" << std::endl;
      return(69);
    }
    EOF

    $CXX main.cpp -o $miq_out/result $CFLAGS
  ]],
}

do
	local libc = {}
	x.libc = libc
	local version = "1.2.3"

  libc.src = x.fetchTar {
		url = f "https://musl.libc.org/releases/musl-{{version}}.tar.gz",
  }

	libc.pkg = x.stdenv {
		name = "musl",
		version = version,
		script = f [[
      {{libc.src}}/configure \
          --prefix=$miq_out \
          --disable-static \
          --enable-wrapper=all \
          --syslibdir="$miq_out/lib"

      make -j$(nproc)
      make install

      ln -vs $miq_out/lib/libc.so $miq_out/bin/ldd
    ]],
	}
end

return x
