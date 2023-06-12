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
  script = f[[
    set -eux
    mkdir -p $miq_out/bin

    for compiler in gcc g++ cpp; do
    tee $miq_out/bin/$compiler <<EOF
    #!{{bootstrap.bootstrap}}/bin/bash
    set -eux
    exec {{bootstrap.bootstrap}}/bin/$compiler $CFLAGS \
      -Wl,-dynamic-linker \
      "\$@" \
      -B{{stage1.libc}}/lib \
      -B{{bootstrap.bootstrap}}/lib \
      -idirafter {{stage1.libc}}/include-libc \
      -idirafter {{bootstrap.bootstrap}}/include-libc \
      -isystem /miq/store/AABB-bootstrap/include-libc
      -isystem {{stage1.libc}}/include-libc \
      -isystem {{bootstrap.bootstrap}}/include-libc
    EOF
    chmod +x $miq_out/bin/$compiler
    done
  ]]
}

stage1.ld = bootstrap.stdenv {
  name = "stage1-ld",
  script = f [[
    mkdir -p $miq_out/bin

    tee $miq_out/bin/ld <<EOF
    #!{{bootstrap.bootstrap}}/bin/bash
    set -eux
    echo "miq ld wrapper"
    exec {{bootstrap.bootstrap}}/bin/ld \
      -dynamic-linker {{stage1.libc}}/lib/ld-musl-x86_64.so.1 \
      "\$@" \
      -rpath {{stage1.libc}}/lib \
      -rpath {{bootstrap.bootstrap}}/lib \
      -L{{stage1.libc}}/lib \
      -L{{bootstrap.bootstrap}}/lib
    EOF
    chmod +x $miq_out/bin/ld
  ]]
}

stage1.stdenv = function(input)
  local stage1 = stage1
  local bootstrap = bootstrap
  input.env = {}
  input.env.PATH = "{{stage1.cc}}/bin:{{stage1.ld}}/bin:{{bootstrap.bootstrap}}/bin"
  input.env["CC"] = "gcc"
  input.env["CXX"] = "g++"
  input.env["LD"] = "ld"

  return miq.package(input)
end

stage1.test_stdenv = stage1.stdenv {
  name = "test_stdenv",
  script = f[[
    set -eux
    printenv
    exit 2
  ]]
}

return stage1
