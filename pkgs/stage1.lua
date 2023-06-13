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

return stage1
