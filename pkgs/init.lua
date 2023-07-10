local miq = require "miq"

local pkgs = {}

pkgs.stage0 = require "stage0"
pkgs.stage1 = require "stage1"

pkgs.empty0 = miq.package {
	name = "empty0",
	script = [[
    set -x
    pwd
    ls -la
    sleep 10
  ]],
}

pkgs.empty1 = miq.package {
	name = "empty1",
	script = [[]],
}

pkgs.empty2 = miq.package {
	name = "empty2",
	script = miq.f [[
    {{pkgs.empty0}}
    {{pkgs.empty1}}
  ]],
}

return pkgs
