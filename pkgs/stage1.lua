local miq = require "miq"
local utils = require "utils"
local f = miq.f

local stage0 = require "stage0"

local x = {}

x.cc = utils.ccBuilder {
	coreutils = stage0.bootstrap,
	shell = stage0.bootstrap,
	cc = f [[
    exec {{stage0.bootstrap}}/bin/$compiler \\
      -pie \\
      -fPIE \\
      -fPIC \\
      -Wformat \\
      -Wformat-security \\
      -Werror=format-security \\
      -fstack-protector-strong \\
      --param ssp-buffer-size=4 \\
      -O2 \\
      -fno-strict-overflow \\
      -Wl,-dynamic-linker={{stage0.libc}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      \$MIQ_CFLAGS
  ]],
}

x.ld = utils.ldBuilder {
	coreutils = stage0.bootstrap,
	shell = stage0.bootstrap,
	ld = f [[
    exec {{stage0.bootstrap}}/bin/ld \\
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
	coreutils = stage0.bootstrap,
	extra = f [[
    export MIQ_CFLAGS="\
    -B{{stage0.libc}}/lib \
    -idirafter {{stage0.libc}}/include \
    -isystem {{stage0.libc}}/include \
    -B{{stage0.libc}}/bin \
    -L{{stage0.libc}}/lib \
    "

    export MIQ_LDFLAGS="\
    -rpath {{stage0.libc}}/lib \
    "
  ]],
}

x.test = x.stdenv {
	name = "test",
	script = f [[
    tee main.c <<EOF
    #include <limits.h>
    #include <stdio.h>
    long foo = LONG_MIN;
    int main() {
      printf("Hello World: %ld", foo);
      return(69);
    }
    EOF
    $CC main.c -o $miq_out/result
  ]],
}

utils.fetchTar = utils.fetchTarBuilder {
	PATH = f "{{stage0.bootstrap}}/bin",
}

do
	local version = "1.4.19"
	local src = utils.fetchTar {
		url = f "https://ftp.gnu.org/gnu/m4/m4-{{version}}.tar.bz2",
	}
	x.m4 = x.stdenv {
		name = "m4",
		version = version,
		script = f [[
      {{src}}/configure \
        --prefix=$miq_out \
        --with-syscmd-shell={{stage0.bootstrap}}

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	local version = "6.2.1"
	local src = utils.fetchTar {
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
	local src = utils.fetchTar {
		url = f "https://ftp.gnu.org/gnu/mpfr/mpfr-{{version}}.tar.bz2",
	}
	x.mpfr = x.stdenv {
		name = "mpfr",
		version = version,
		depend = {
			x.gmp,
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
	local src = utils.fetchTar {
		url = f "https://ftp.gnu.org/gnu/mpc/mpc-{{version}}.tar.gz",
	}
	x.libmpc = x.stdenv {
		name = "libmpc",
		version = version,
		depend = {
			x.gmp,
			x.mpfr,
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

do
	local version = "12.2.0"
	local patches = {
		no_sys_dirs = miq.fetch {
			url = "https://raw.githubusercontent.com/NixOS/nixpkgs/ddf4688dc7aeb14e8a3c549cb6aa6337f187a884/pkgs/development/compilers/gcc/gcc-12-no-sys-dirs.patch",
		},
	}
	local src = utils.fetchTar {
		url = f "https://ftp.gnu.org/gnu/gcc/gcc-{{version}}/gcc-{{version}}.tar.gz",
		post = f [[
      set -ex
      patch -p1 < {{patches.no_sys_dirs}}
      sed -i gcc/config/linux.h -e '1i#undef LOCAL_INCLUDE_DIR'
    ]],
	}
	x.gcc = x.stdenv {
		name = "gcc",
		version = version,
		depend = {
			x.gmp,
			x.mpfr,
			x.libmpc,
		},
		script = f [[
      set -ex
      mkdir -p $miq_out/build
      cd $miq_out/build

      {{src}}/configure \
        --prefix="$PREFIX" \
        --disable-multilib \
        --disable-bootstrap \
        --disable-libmpx \
        --disable-libsanitizer \
        --disable-symvers \
        --disable-libcc1 \
        libat_cv_have_ifunc=no \
        --disable-gnu-indirect-function \
        --with-gmp-include={{x.gmp}}/include \
        --with-gmp-lib={{x.gmp}}/lib \
        --with-mpfr-include={{x.mpfr}}/include \
        --with-mpfr-lib={{x.mpfr}}/lib \
        --with-mpc-include={{x.libmpc}}/include \
        --with-mpc-lib={{x.libmpc}}/lib \
        --with-native-system-header-dir={{stage0.libc}}/include \
        --with-build-sysroot=/

        makeFlags="\
        NATIVE_SYSTEM_HEADER_DIR={{stage0.libc}}/include \
        SYSTEM_HEADER_DIR={{stage0.libc}} \
        BUILD_SYSTEM_HEADER_DIR={{stage0.libc}} \
        "

        make $makeFlags
        make $makeFlags -j$(nproc) install
      ]],
	}
end
--disable-bootstrap \
--disable-nls \
--enable-languages=c,c++ \

do
	local version = "0.5.12"
	local src = utils.fetchTar {
		url = f "http://gondor.apana.org.au/~herbert/dash/files/dash-{{version}}.tar.gz",
	}
	x.dash = x.stdenv {
		name = "dash",
		version = version,
		script = f [[
      {{src}}/configure --prefix=$PREFIX

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	local version = "0.5.11"
	local src = utils.fetchTar {
		url = f "http://gondor.apana.org.au/~herbert/dash/files/dash-{{version}}.tar.gz",
	}
	x.dash_mod = x.stdenv {
		name = "dash",
		version = version,
		script = f [[
      {{src}}/configure --prefix=$PREFIX

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

return x
