local miq = require("miq")
local package = miq.package
local f = miq.f

---@param first table<string, any>
---@param second table<string, any>
local merge = function(first, second)
	for k, v in pairs(second) do
		if first[k] == nil then
			first[k] = v
		else
			error("Tried to merge two tables with same key: " .. k, 2)
		end
	end
end

local pkgs = {}

merge(pkgs, require("bootstrap"))
merge(pkgs, require("stage1"))

miq.trace(pkgs)

return pkgs
