local miq = require ("miq")
local package = miq.package
local f = miq.f

---@param first table
---@param second table
local merge = function(first, second)
  for k,v in pairs(second) do first[k] = v end
end


local pkgs = {}

merge(pkgs, require("bootstrap"))


pkgs.test = package {
  name = "test",
  script =  f[[
    set -x
    echo $FOO
    echo $FOO2
    exit 2
  ]],
  env = {
    FOO = "bar",
    FOO2 = f"{{pkgs.busybox}}"
  }
}




miq.trace(pkgs)


return pkgs
