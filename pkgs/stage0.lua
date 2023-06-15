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
      -pie \\
      -fPIE \\
      -fPIC \\
      -Wformat \\
      -Wformat-security \\
      -Werror=format-security \\
      -fstack-protector-strong \\
      --param ssp-buffer-size=4 \\
      -O2 \\
      -U_FORTIFY_SOURCE \\
      -D_FORTIFY_SOURCE=2 \\
      -fno-strict-overflow \\
      -Wl,-dynamic-linker={{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      \$MIQ_CFLAGS
  ]],
}

x.ld = utils.ldBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	ld = f [[
    exec {{x.bootstrap}}/bin/ld \\
      -z relro \\
      -pie \\
      -z now \\
      "\$@" \\
      \$MIQ_LDFLAGS
  ]],
}

x.stdenv = utils.stdenvBuilder {
	name = "stage0-stdenv",
	cc = x.cc,
	ld = x.ld,
	coreutils = x.bootstrap,
	extra = f [[
    export MIQ_CFLAGS="\
    -B{{x.bootstrap}}/lib \
    -idirafter {{x.bootstrap}}/include-libc \
    -idirafter {{x.bootstrap}}/lib/gcc/x86_64-unknown-linux-musl/7.3.0/include-fixed \
    -B{{x.bootstrap}}/bin \
    -Wl,-rpath \
    -L{{x.bootstrap}}/lib \
    -L{{x.bootstrap}}/lib/gcc/x86_64-unknown-linux-musl/7.3.0 \
    "

    export MIQ_LDFLAGS="\
    -rpath {{x.bootstrap}}/lib \
    "
  ]],
}
-- -plugin-opt=-pass-through=-lgcc \
-- -plugin-opt=-pass-through=-lgcc_s \
-- -plugin-opt=-pass-through=-lc \
-- -plugin-opt=-pass-through=-lgcc \
-- -plugin-opt=-pass-through=-lgcc_s \
-- --eh-frame-hdr \
-- -m elf_x86_64 \
-- -dynamic-linker {{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \
-- -pie \
-- {{x.bootstrap}}/lib/Scrt1.o \
-- {{x.bootstrap}}/lib/crti.o \
-- {{x.bootstrap}}/lib/gcc/x86_64-unknown-linux-musl/7.3.0/crtbegin.o \
-- -L{{x.bootstrap}}/lib \
-- -L{{x.bootstrap}}/lib/gcc/x86_64-unknown-linux-musl/7.3.0 \
-- -dynamic-linker={{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \
-- -lgcc \
-- --push-state \
-- --as-needed \
-- -lgcc_s \
-- --pop-state \
-- -lc \
-- -lgcc \
-- --push-state \
-- --as-needed \
-- -lgcc_s \
-- --pop-state \

x.fetchTar = utils.fetchTarBuilder {
	PATH = f "{{x.bootstrap}}/bin",
}

x.test = x.stdenv {
	name = "test",
	script = f [[
    tee main.c <<EOF
    int main() { return(69); }
    EOF
    $CC main.c -o $miq_out/result
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
			x.m4,
		},
		script = f [[
      {{src}}/configure \
        --prefix=$PREFIX \
        --with-pic

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	local version = "4.2.0"
	local src = x.fetchTar {
		url = f "https://ftp.gnu.org/gnu/mpfr/mpfr-{{version}}.tar.bz2",
	}
	x.mpfr = x.stdenv {
		name = "mpfr",
		version = version,
		depend = {
      x.gmp
		},
		script = f [[
      {{src}}/configure \
        --prefix=$PREFIX \
        --with-pic

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	local version = "1.3.1"
	local src = x.fetchTar {
		url = f "https://ftp.gnu.org/gnu/mpc/mpc-{{version}}.tar.gz",
	}
	x.libmpc = x.stdenv {
		name = "libmpc",
		version = version,
		depend = {
      x.gmp,
      x.mpfr
		},
		script = f [[
      {{src}}/configure \
        --prefix="$PREFIX" \
        --disable-dependency-tracking \
        --with-pic

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
