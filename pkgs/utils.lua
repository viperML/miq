---@type {package: fun(table): Package}
local miq = require "miq"

local f = miq.f

local x = {}

---@alias Package
---| { result: string, name: string, deps: table<string>, script: string }

---@param input { cc: Package, shell: Package, coreutils: Package }
---@return Package
x.ccBuilder = function(input)
	local cc = input.cc
	local shell = input.shell
	local coreutils = input.coreutils
	local result = miq.package {
		name = f "{{input.cc.name}}-cc-wrapper",
		env = {
			PATH = f "{{coreutils}}/bin",
		},
		script = f [[
      set -eux
      mkdir -p $miq_out/bin

      for compiler in gcc g++ cpp; do

      tee $miq_out/bin/$compiler <<EOF
      #!{{shell}}/bin/bash

      set -x
      exec {{cc}}/bin/$compiler \\
        -Wl,-dynamic-linker={{cc}}/lib/ld-musl-x86_64.so.1 \\
        "\$@" \\
        -B{{cc}}/lib \\
        -idirafter {{cc}}/include \\
        -isystem {{cc}}/include
      EOF

      chmod +x $miq_out/bin/$compiler
      done
    ]],
	}
	return result
end

---@param input { ld: Package, shell: Package, coreutils: Package }
---@return Package
x.ldBuilder = function(input)
	local ld = input.ld
	local shell = input.shell
	local coreutils = input.coreutils
	local result = miq.package {
		name = f "{{input.ld.name}}-ld-wrapper",
		env = {
			PATH = f "{{coreutils}}/bin",
		},
		script = f [[
      mkdir -p $miq_out/bin

      tee $miq_out/bin/ld <<EOF
      #!{{shell}}/bin/bash
      echo "MIQ_LDFLAGS: \$MIQ_LDFLAGS" >&2
      set -x

      exec {{ld}}/bin/ld \\
        -dynamic-linker {{ld}}/lib/ld-musl-x86_64.so.1 \\
        "\$@" \\
        -rpath {{ld}}/lib \\
        -L{{ld}}/lib \\
        \$MIQ_LDFLAGS
      EOF

      chmod +x $miq_out/bin/ld
    ]],
	}
	return result
end

---@param input {cc: Package, ld: Package, name: string, coreutils: Package, extra: any}
---@return fun(table): Package
x.stdenvBuilder = function(input)
	local input = input
	local pkg = miq.package {
		name = input.name,
		env = {
			PATH = f "{{input.coreutils}}/bin",
		},
		script = f [[
      set -x
      tee $miq_out/stdenv.sh <<EOF
      echo "stdenv setup" >&2
      export PATH="{{input.cc}}/bin:{{input.ld}}/bin:{{input.coreutils}}/bin"

      export CC="gcc"
      export CXX="g++"
      export CFLAGS="-O2 -pipe -pie -fPIE -fPIC"

      export LD="ld"

      {{input.extra}}
      EOF
    ]],
	}

	---@param args {depend: table<Package>, script: any}
	---@return Package
	local result = function(args)
		local args = args
    local pkg = pkg

		args.script = f [[
      source {{pkg}}/stdenv.sh
      set -x
      set -e
      printenv

      {{args.script}}
    ]]

		return miq.package(args)
	end

	return result
end

x.fetchTarBuilder = function(input)
  local input = input

  local fn_result = function(args)
    local args = args
    local input = input

    local fetch = miq.fetch(args)
    local pkg = miq.package {
      name = f"{{fetch.name}}-unpack",
      script = f[[
        export PATH="{{input.PATH}}"
        cd $miq_out
        tar -xvf {{fetch}} --strip-components=1 --no-same-permissions --no-same-owner
      ]]
    }
    return pkg
  end

  return fn_result
end

return x
