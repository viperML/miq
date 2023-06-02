local miq = require "miq"
local inspect = require "inspect"
local fetch = miq.fetch
local package = miq.package
-- local f = miq.f
local trace = miq.trace

local pkgs = {}

pkgs.bootstrap_tools = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/bootstrap-tools.tar.xz"
}

pkgs.busybox = fetch {
  url = "https://wdtz.org/files/gywxhjgl70sxippa0pxs0vj5qcgz1wi8-stdenv-bootstrap-tools/on-server/busybox",
  executable = true
}

-- pkgs.toybox = fetch {
--     url = "http://landley.net/toybox/bin/toybox-x86_64",
--     executable = true
-- }

pkgs.unpack_bootstrap_tools = fetch {
  url = "https://raw.githubusercontent.com/NixOS/nixpkgs/d6b863fd9b7bb962e6f9fdf292419a775e772891/pkgs/stdenv/linux/bootstrap-tools-musl/scripts/unpack-bootstrap-tools.sh",
  executable = true
}

pkgs.foo = package {
  name = "foo",
  version = "1.0",
  deps = {
    -- pkgs.bootstrap_tools,
    -- pkgs.toybox
  },
  script = [[
  set -x
  ls -lv {{pkgs.busybox}}
  ]],
  env = {
    FOO = "bar",
    FOOO = "baar",
  }
}

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
      local match_result = tostring(fn())
      trace(match_result)
      -- Append to result.deps list

      table.insert(result.deps, match_result)

      return match_result
    else
      error(err, 0)
    end
  end)

  result.value = substituted

  return result
end

local a  = "xd"

local test = f[[
text
{{ pkgs.foo }}
text
]]

trace(test)
miq.parse_metatext(test)

return pkgs
