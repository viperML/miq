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

x.cc = utils.ccBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	cc = f [[
    exec {{x.bootstrap}}/bin/$compiler \\
      -Wl,-dynamic-linker={{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -O2 -pipe -pie -fPIE -fPIC \\
      -B{{x.bootstrap}}/lib \\
      -idirafter {{x.bootstrap}}/include \\
      -isystem {{x.bootstrap}}/include \\
      -idirafter {{x.bootstrap}}/include-libc \\
      -isystem {{x.bootstrap}}/include-libc
  ]],
}

x.ld = utils.ldBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	ld = f [[
    exec {{x.bootstrap}}/bin/ld \\
      -dynamic-linker {{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -rpath {{x.bootstrap}}/lib \\
      -L{{x.bootstrap}}/lib
  ]],
}

x.stdenv = utils.stdenvBuilder {
	name = "stage0-stdenv",
	cc = x.cc,
	ld = x.ld,
	coreutils = x.bootstrap,
	extra = "",
}

x.fetchTar = utils.fetchTarBuilder {
	PATH = f "{{x.bootstrap}}/bin",
}

x.test = x.stdenv {
	name = "test",
	script = f [[
    tee main.c <<EOF
    int main() { return(69); }
    EOF
    $CC $CFLAGS main.c -o $miq_out/result
  ]],
}

-- do
-- 	local version = "1.2.3"
-- 	local src = x.fetchTar {
-- 		url = f "https://musl.libc.org/releases/musl-{{version}}.tar.gz",
-- 	}
-- 	x.libc = x.stdenv {
-- 		name = "musl",
-- 		version = version,
-- 		script = f [[
--       {{src}}/configure \
--           --prefix=$miq_out \
--           --disable-static \
--           --enable-wrapper=all \
--           --syslibdir="$miq_out/lib"

--       make -j$(nproc)
--       make install

--       ln -vs $miq_out/lib/libc.so $miq_out/bin/ldd
--     ]],
-- 	}
-- end

do
	local version = "1.4.19"
	local src = x.fetchTar {
		url = f "https://ftp.gnu.org/gnu/m4/m4-{{version}}.tar.bz2",
	}
	x.m4 = x.stdenv {
		name = "m4",
		version = version,
		script = f [[
      {{src}}/configure \
        --prefix=$miq_out \
        --with-syscmd-shell={{x.bootstrap}}

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	local version = "6.2.1"
	local src = x.fetchTar {
		url = f "https://ftp.gnu.org/gnu/gmp/gmp-{{version}}.tar.bz2",
	}
	x.gmp = x.stdenv {
		name = "gmp",
		version = version,
    depend = {
      x.m4
    },
		script = f [[
      {{src}}/configure \
        --prefix=$PREFIX \
        --with-pic \
        --with-cxx

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

-- do
-- 	local version = "12.2.0"
-- 	local src = x.fetchTar {
-- 		url = f "https://ftp.gnu.org/gnu/gcc/gcc-{{version}}/gcc-{{version}}.tar.gz",
-- 	}
-- 	x.gcc_src = src
-- 	x.gcc = x.stdenv {
-- 		name = "gcc",
-- 		version = version,
-- 		script = f [[
--       mkdir -p $miq_out/build
--       cd $miq_out/build
--       {{src}}/configure \
--         --prefix="$PREFIX" \
--         --disable-nls \
--         --enable-languages=c,c++ \
--         --disable-multilib \
--         --disable-bootstrap \
--         --disable-libmpx \
--         --disable-libsanitizer \
--         --with-gmp-include={{x.bootstrap}}/include \
--         --with-gmp-lib={{x.bootstrap}}/lib \
--         --with-mpfr-include={{x.bootstrap}}/include \
--         --with-mpfr-lib={{x.bootstrap}}/lib \
--         --with-mpc-include={{x.bootstrap}}/include \
--         --with-mpc-lib={{x.bootstrap}}/lib
--     ]],
-- 	}
-- end

return x
