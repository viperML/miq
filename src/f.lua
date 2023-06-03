local miq = require("miq")

---@alias Metatext
---| { value: string, deps: string[] }

---@param str string
---@return Metatext substituted
local f = function(str)
  local outer_env = _ENV

  local result = {}
  result.deps = {}

  local substituted = str:gsub("%b{}", function(block)
    local code = block:match("{{(.*)}}")
    local exp_env = {}
    setmetatable(exp_env, { __index = function(_, k)
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
    end })
    local fn, err = load("return "..code, "expression `"..code.."`", "t", exp_env)
    if fn then
      local match_result = fn()
      match_result = miq.get_result(match_result)
      -- Append to result.deps list
      table.insert(result.deps, match_result)
      return "/miq/store/"..match_result
    else
      error(err, 0)
    end
  end)

  result.value = substituted

  return result
end

return f
