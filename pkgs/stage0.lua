local miq = require "miq"
local utils = require "utils"

local f = miq.f

local x = {}

x.bootstrap_tools = miq.fetch {
	url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz",
}


x.unpack_bootstrap_tools = miq.fetch {
	url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
	executable = true,
}

x.bootstrap = miq.package {
	name = "bootstrap",
	script = f [[
    set -exu

    export out="$miq_out"
    export tarball={{x.bootstrap_tools}}
    export builder=/usr/bin/busybox
    {{x.unpack_bootstrap_tools}}
  ]],
}

x.cc = utils.ccBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	cc = f [[
    exec {{x.bootstrap}}/bin/$compiler \\
      -fPIC \\
      -Wformat \\
      -Wformat-security \\
      -Werror=format-security \\
      -fstack-protector-strong \\
      --param ssp-buffer-size=4 \\
      -O2 \\
      -fno-strict-overflow \\
      -Wl,-dynamic-linker={{x.bootstrap}}/lib/ld-musl-x86_64.so.1 \\
      "\$@" \\
      \$MIQ_CFLAGS
  ]],
}

-- -pie \\
-- -fPIE \\
-- -U_FORTIFY_SOURCE \\
-- -D_FORTIFY_SOURCE=2 \\

x.ld = utils.ldBuilder {
	coreutils = x.bootstrap,
	shell = x.bootstrap,
	ld = f [[
    exec {{x.bootstrap}}/bin/ld \\
      -z relro \\
      -z now \\
      "\$@" \\
      \$MIQ_LDFLAGS
  ]],
}
-- -pie \\

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
    -L{{x.bootstrap}}/lib \
    -L{{x.bootstrap}}/lib/gcc/x86_64-unknown-linux-musl/7.3.0 \
    "

    export MIQ_LDFLAGS="\
    -rpath {{x.bootstrap}}/lib \
    "
  ]],
}

-- -Wl,-rpath \
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

do
	local version = "1.2.3"
	local src = x.fetchTar {
		url = f "https://musl.libc.org/releases/musl-{{version}}.tar.gz",
	}
	x.libc = x.stdenv {
		name = "musl",
		version = version,
		script = f [[
      {{src}}/configure \
          --prefix=$PREFIX \
          --disable-static \
          --enable-wrapper=all \
          --syslibdir=$PREFIX/lib

      make -j$(nproc)
      make -j$(nproc) install

      ln -vs $miq_out/lib/libc.so $miq_out/bin/ldd
    ]],
	}
end

return x
