local miq = require "miq"

---@alias MetaText
---| { value: string, deps: string[] }

---@alias MetaTextInput
---| MetaText
---| string

---@param raw_text string
---@return MetaTextInput
local f = function(raw_text)
	local outer_env = _ENV
  raw_text = miq.dedent(raw_text)

	local result = {}
	result.deps = {}

	local substituted = raw_text:gsub("%b{}", function(block)
		local code = block:match "{{(.*)}}"
		-- Workaround: we are matching {FOO}, skip if we do
		-- miq.trace("Got block: "..block)
		-- Check if code is nil
		if code == nil then
			return block
		end

		local exp_env = {}
		setmetatable(exp_env, {
			__index = function(_, k)
				local stack_level = 5
				while debug.getinfo(stack_level, "") ~= nil do
					local i = 1
					repeat
						local name, value = debug.getlocal(stack_level, i)
						if name == k then
							return value
						end
						i = i + 1
					until name == nil
					stack_level = stack_level + 1
				end
				return rawget(outer_env, k)
			end,
		})

		local fn, err = load("return " .. code, "expression `" .. code .. "`", "t", exp_env)

		if fn then
			local lua_value = fn()

			local text, d = miq.interpolate(lua_value)

			if d ~= nil then
				if d[1] ~= nil then
					-- d is a list of deps
					for _, dep in ipairs(d) do
						table.insert(result.deps, dep)
					end
				else
					-- d is a single dep
					table.insert(result.deps, d)
				end
			end

      miq.trace(result)

			return text
		else
			error(err, 0)
		end
	end)

	result.value = substituted

	-- Serde doesn't like an empty list for deser
	-- Enum variant fails if deps is empty
	if next(result.deps) == nil then
		return result.value
	else
		return result
	end
end

return f
