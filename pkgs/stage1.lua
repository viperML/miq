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
    echo "MIQ_CFLAGS: \$MIQ_CFLAGS" >&2

    set -x
    exec {{bootstrap.bootstrap}}/bin/$compiler \\
      -Wl,-dynamic-linker={{stage1.libc}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -B{{stage1.libc}}/lib \\
      -idirafter {{stage1.libc}}/include \\
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
    echo "MIQ_LDFLAGS: \$MIQ_LDFLAGS" >&2

    exec {{bootstrap.bootstrap}}/bin/ld \\
      -dynamic-linker {{stage1.libc}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      -rpath {{stage1.libc}}/lib \\
      -L{{stage1.libc}}/lib \\
      \$MIQ_LDFLAGS
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
	local cflags = "-O2 -pipe -pie -fPIE -fPIC"

	if input.depend ~= nil then
		local metatexti = {
			deps = {},
			value = cflags,
		}
		for i, dep in ipairs(input.depend) do
			local dep = dep
			local m = f " -B{{dep}}/lib -idirafter {{dep}}/include -isystem {{dep}}/include"
			for _, d in ipairs(m.deps) do
				table.insert(metatexti.deps, d)
			end
			metatexti.value = metatexti.value .. m.value
		end
		input.env["CFLAGS"] = metatexti
	else
		input.env["CFLAGS"] = cflags
	end
	input.depend = nil

	miq.trace(input)
	return miq.package(input)
end

-- stage1.trivial = stage1.stdenv {
-- 	name = "trivial",
-- 	depend = {
-- 		bootstrap.bootstrap,
-- 	},
-- 	script = f [[
--     gcc --version
--     exit 2
--   ]],
-- }

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

do
	stage1.mpfr = {}
	local version = "4.2.0"
	local src = fetch {
		url = f "https://ftp.gnu.org/gnu/mpfr/mpfr-{{version}}.tar.bz2",
	}
	stage1.mpfr.src = bootstrap.stdenv {
		name = "mpfr_src",
		version = version,
		script = f [[
      mkdir $miq_out
      cd $miq_out
      tar -xvf {{src}} --strip-components=1 --no-same-permissions --no-same-owner
    ]],
	}
	stage1.mpfr.pkg = stage1.stdenv {
		name = "mpfr",
		version = version,
		depend = {
			stage1.gmp.pkg,
		},
		script = f [[
      mkdir $miq_out
      export PREFIX=$miq_out
      {{stage1.mpfr.src}}/configure \
        --prefix="$PREFIX" \
        --with-pic
      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

do
	stage1.libmpc = {}
	local version = "1.3.1"
	local src = fetch {
		url = f "https://ftp.gnu.org/gnu/mpc/mpc-{{version}}.tar.gz",
	}
	stage1.libmpc.src = bootstrap.stdenv {
		name = "libmpc_src",
		version = version,
		script = f [[
      mkdir $miq_out
      cd $miq_out
      tar -xvf {{src}} --strip-components=1 --no-same-permissions --no-same-owner
    ]],
	}
	stage1.libmpc.pkg = stage1.stdenv {
		name = "libmpc",
		version = version,
		depend = {
			stage1.gmp.pkg,
			stage1.mpfr.pkg,
		},
		script = f [[
      set -x
      mkdir -p $miq_out
      export PREFIX=$miq_out
      {{stage1.libmpc.src}}/configure \
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
	local src_raw = fetch {
		url = f "https://mirrorservice.org/sites/sourceware.org/pub/gcc/releases/gcc-{{version}}/gcc-{{version}}.tar.xz",
	}
	stage1.gcc = {}
	stage1.gcc.src = bootstrap.stdenv {
		name = "gcc_src",
		version = version,
		script = f [[
      mkdir $miq_out
      cd $miq_out
      tar -xvf {{src_raw}} --strip-components=1 --no-same-permissions --no-same-owner
    ]],
	}
	stage1.gcc.pkg = stage1.stdenv {
		name = "gcc",
		version = version,
		depend = {
			stage1.gmp.pkg,
			stage1.mpfr.pkg,
			stage1.libmpc.src,
		},
		script = f [[
      export PREFIX=$miq_out
      mkdir $miq_out

      {{stage1.gcc.src}}/configure \
        --prefix="$PREFIX" \
        --disable-nls \
        --enable-languages=c,c++ \
        --disable-bootstrap

      make -j$(nproc)
      make install -j$(nproc)
    ]],
	}
end

return stage1
