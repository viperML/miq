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
      -Wl,-dynamic-linker={{stage0.libc}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -O2 -pipe -pie -fPIE -fPIC \\
      -B{{stage0.libc}}/lib \\
      -idirafter {{stage0.libc}}/include \\
      -isystem {{stage0.libc}}/include
  ]],
}

x.ld = utils.ldBuilder {
	coreutils = stage0.bootstrap,
	shell = stage0.bootstrap,
	ld = f [[
    exec {{stage0.bootstrap}}/bin/ld \\
      -dynamic-linker {{stage0.libc}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -rpath {{stage0.libc}}/lib \\
      -L{{stage0.libc}}/lib
  ]],
}

x.stdenv = utils.stdenvBuilder {
	name = "stage1-stdenv",
	cc = x.cc,
	ld = x.ld,
	coreutils = stage0.bootstrap,
	extra = "",
}

x.fetchTar = stage0.fetchTar

-- x.test = x.stdenv {
-- 	name = "test",
-- 	script = f [[
--     tee main.c <<EOF
--     int main() { return(69); }
--     EOF
--     $CC $CFLAGS main.c -o $miq_out/result
--   ]],
-- }



return x
