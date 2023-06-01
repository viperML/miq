local miq = require "miq"
local inspect = require "inspect"
local fetch = miq.fetch
local package = miq.package

local pkgs = {}

pkgs.bootstrap_tools = fetch {
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
}

pkgs.busybox = fetch {
    url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
    executable = true
}

pkgs.toybox = fetch {
    -- FIXME use static url
    url = "http://landley.net/toybox/bin/toybox-x86_64",
    executable = true
}

pkgs.unpack_bootstrap_tools = fetch {
    url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
    executable = true
}

pkgs.foo = package {
    name = "foo",
    version = "1.0",
    deps = {
        pkgs.bootstrap_tools,
        pkgs.toybox
    },
    script = [[
        set -x
        echo "Hello world"
    ]],
    env = {
        FOO = "bar",
        FOOO = "baar",
    }
}

pkgs.bar = package {
    name = "bar",
    deps = {
        pkgs.foo
    }
}


return pkgs
