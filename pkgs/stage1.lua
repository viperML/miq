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

return stage1
