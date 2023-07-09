---@type {package: fun(table): Package}
local miq = require "miq"

local f = miq.f

local x = {}

---@alias Package
---| { result: string, name: string, deps: table<string>, script: string }

x.ccBuilder = function(input)
	local input = input
	local result = miq.package {
		name = "cc-wrapper",
		env = {
			PATH = f "{{input.coreutils}}/bin",
		},
		script = f [[
      set -eux
      mkdir -p $miq_out/bin

      for compiler in gcc g++ cpp; do
      tee $miq_out/bin/$compiler <<EOF
      #!{{input.shell}}/bin/bash
      echo MIQ_CFLAGS: \$MIQ_CFLAGS 1>&2
      set -x
      {{input.cc}}
      EOF
      chmod +x $miq_out/bin/$compiler
      done
    ]],
	}
	return result
end

x.ldBuilder = function(input)
	local input = input
	local result = miq.package {
		name = "ld-wrapper",
		env = {
			PATH = f "{{input.coreutils}}/bin",
		},
		script = f [[
      mkdir -p $miq_out/bin

      tee $miq_out/bin/ld <<EOF
      #!{{input.shell}}/bin/bash
      echo MIQ_LDFLAGS: \$MIQ_LDFLAGS 1>&2
      set -x
      {{input.ld}}
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
      mkdir -p $miq_out
      tee $miq_out/stdenv.sh <<EOF
      echo "stdenv setup" >&2
      export PATH="{{input.cc}}/bin:{{input.ld}}/bin:{{input.coreutils}}/bin"
      export PREFIX=\$miq_out

      export CC="gcc"
      export CXX="g++"
      # export CFLAGS="-O2 -pipe -pie -fPIE -fPIC"

      export LD="ld"

      export NIX_DEBUG=1

      mkdir -p \$miq_out

      {{input.extra}}
      EOF
    ]],
	}

	---@param args {depend: table<Package>, script: any}
	---@return Package
	local result = function(args)
		local args = args
		local pkg = pkg

		local extra_script = {
			deps = {},
			value = "",
		}

		if args.depend ~= nil then
			for _, dep in ipairs(args.depend) do
				local dep = dep
				local text = f [[
          export MIQ_CFLAGS="$MIQ_CFLAGS -isystem {{dep}}/include -L{{dep}}/lib"
          export MIQ_LDFLAGS="$MIQ_LDFLAGS -L{{dep}}/lib"
          export PATH="{{dep}}/bin:$PATH"
        ]]
				for _, d in ipairs(text.deps) do
					table.insert(extra_script.deps, d)
				end
				extra_script.value = extra_script.value .. text.value
			end
			-- input.env["CFLAGS"] = metatexti
		end
		args.depend = nil

		if not (extra_script.deps[1] ~= nil) then
			extra_script = extra_script.value
		end

		args.script = f [[
      source {{pkg}}/stdenv.sh
      set -x
      set -e
      {{extra_script}}
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

		local post
		if args.post ~= nil then
			post = args.post
		else
			post = "# No post unpack"
		end

		local fetch = miq.fetch(args)
		local pkg = miq.package {
			name = f "{{fetch.name}}-unpack",
			script = f [[
        set -ex
        export PATH="{{input.PATH}}"
        mkdir -p $PREFIX
        cd $PREFIX
        tar -xvf {{fetch}} --strip-components=1 --no-same-permissions --no-same-owner

        {{post}}
      ]],
		}
		return pkg
	end

	return fn_result
end

return x
