local miq = require "miq"
local package = miq.package
local fetch = miq.fetch
local f = miq.f
local trace = miq.trace

local bootstrap = require "bootstrap"

local stage1 = {}

stage1.libc_src = fetch {
	url = "https://musl.libc.org/releases/musl-1.2.3.tar.gz",
}

stage1.libc = bootstrap.stdenv {
	name = "libc",
	version = "1.2.3",
	script = f [[
    set -exu
    tar -xvf {{stage1.libc_src}} --strip-components=1

    ./configure \
        --prefix=$miq_out \
        --disable-static \
        --enable-wrapper=all \
        --syslibdir="$miq_out/lib"

    make -j$(nproc)
    make install

    ln -vs $miq_out/lib/libc.so $miq_out/bin/ldd
  ]],
}

stage1.cc = bootstrap.stdenv {
	name = "stage1-cc",
	script = f [[
    set -eux
    mkdir -p $miq_out/bin

    for compiler in gcc g++ cpp; do
    tee $miq_out/bin/$compiler <<EOF
    #!{{bootstrap.bootstrap}}/bin/bash
    set -eux
    exec {{bootstrap.bootstrap}}/bin/$compiler \
      $CFLAGS \
      -Wl,-dynamic-linker={{stage1.libc}}/lib/ld-musl-x86_64.so.1 \
      "\$@" \
      -B{{stage1.libc}}/lib \
      -idirafter {{stage1.libc}}/include \
      -isystem {{stage1.libc}}/include
    EOF
    chmod +x $miq_out/bin/$compiler
    done
  ]],
}

stage1.ld = bootstrap.stdenv {
	name = "stage1-ld",
	script = f [[
    mkdir -p $miq_out/bin

    tee $miq_out/bin/ld <<EOF
    #!{{bootstrap.bootstrap}}/bin/bash
    set -eux
    echo "miq ld wrapper running"

    exec {{bootstrap.bootstrap}}/bin/ld \
      -dynamic-linker {{stage1.libc}}/lib/ld-musl-x86_64.so.1 \
      "\$@" \
      -rpath {{stage1.libc}}/lib \
      -L{{stage1.libc}}/lib \
    EOF

    chmod +x $miq_out/bin/ld
  ]],
}

stage1.stdenv = function(input)
	local stage1 = stage1
	local bootstrap = bootstrap
	input.env = {}
	input.env.PATH = f "{{stage1.cc}}/bin:{{stage1.ld}}/bin:{{bootstrap.bootstrap}}/bin"
	input.env["CC"] = "gcc"
	input.env["CXX"] = "g++"
	input.env["LD"] = "ld"
	input.env["CFLAGS"] = "-O2 -pipe -pie -fPIE -fPIC"

	return miq.package(input)
end

stage1.trivial = stage1.stdenv {
	name = "trivial",
	script = f [[
    tee main.c <<EOF
    #include <stdio.h>
    #include <stdlib.h>
    int main() {
      printf("Hello World");
      return(0);
    }
    EOF

    mkdir -p $miq_out/bin
    $CC main.c -o $miq_out/bin/hello
  ]],
}

local dash_src = fetch {
	url = "http://gondor.apana.org.au/~herbert/dash/files/dash-0.5.12.tar.gz",
}

stage1.dash_src = stage1.stdenv {
	name = "dash_src",
	script = f [[
    set -eux
    mkdir -pv $miq_out
    cd $miq_out

    tar -xvf {{dash_src}} --strip-components=1 --no-same-permissions --no-same-owner
  ]],
}

stage1.dash = stage1.stdenv {
	name = "dash",
	script = f [[
    {{stage1.dash_src}}/configure --prefix=$miq_out
    ls -la

    make -j$(nproc)
    make install -j$(nproc)
  ]],
}

local m4 = {}
m4.version = "1.4.19"
m4.src = fetch {
	url = f "https://ftp.gnu.org/gnu/m4/m4-{{m4.version}}.tar.bz2",
}
m4.pkg = stage1.stdenv {
	name = "m4",
	version = m4.version,
	script = f [[
    tar -xvf {{m4.src}} --strip-components=1 --no-same-permissions --no-same-owner
    mkdir $miq_out
    ./configure --prefix=$miq_out --with-syscmd-shell={{bootstrap.bootstrap}}
    make -j$(nproc)
    make install -j$(nproc)
  ]],
}
stage1.m4 = m4

local gmp = {}
gmp.version = "6.2.1"
gmp.src = fetch {
	url = f "https://ftp.gnu.org/gnu/gmp/gmp-{{gmp.version}}.tar.bz2",
}
gmp.pkg = stage1.stdenv {
	name = "gmp",
	version = gmp.version,
	script = f [[
    export PATH="{{m4.pkg}}/bin:$PATH"
    tar -xvf {{gmp.src}} --strip-components=1 --no-same-permissions --no-same-owner
    mkdir $miq_out
    ./configure --prefix=$miq_out --with-pic --with-cxx
    make -j$(nproc)
    make install -j$(nproc)
  ]],
}
stage1.gmp = gmp

local gcc_version = "12.2.0"

local gcc_src_raw = fetch {
	url = f "https://mirrorservice.org/sites/sourceware.org/pub/gcc/releases/gcc-{{gcc_version}}/gcc-{{gcc_version}}.tar.xz",
}

stage1.gcc_src = stage1.stdenv {
	name = "gcc_src",
	version = gcc_version,
	script = f [[
    mkdir $miq_out
    cd $miq_out

    tar -xvf {{gcc_src_raw}} --strip-components=1 --no-same-permissions --no-same-owner
  ]],
}

stage1.gcc = stage1.stdenv {
	name = "gcc",
	version = gcc_version,
	script = f [[
    export PREFIX=$miq_out
    mkdir $miq_out
    cd $miq_out

    {{stage1.gcc_src}}/configure \
      --prefix="$PREFIX" \
      --disable-nls \
      --enable-languages=c,c++

    make -j$(nproc)
    make install -j$(nproc)
  ]],
}

return stage1
