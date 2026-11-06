local dir = os.getenv("COVERAGE_DIR")
if not dir then
  return
end

local runner = require("luacov.runner")
local reporter = require("luacov.reporter")
local lcov = require("luacov.reporter.lcov")

local function slashes(path)
  return (path:gsub("\\", "/"))
end

local runfiles = require("runfiles")
local prefix = os.getenv("TEST_WORKSPACE") .. "/"

-- Covered files are named by their runfiles key, so the names are the same with or without a
-- runfiles tree (Windows has only a manifest, and its paths point into the execroot).
local names = {}
local include = {}
for _, key in ipairs(runfiles.sources()) do
  local path = key:sub(1, #prefix) == prefix and runfiles.rlocation(key)
  if path then
    path = slashes(path)
    names[path] = key:sub(#prefix + 1)
    include[#include + 1] = "^" .. path:gsub("%.lua$", ""):gsub("%p", "%%%0") .. "$"
  end
end
if not include[1] then
  include[1] = "^$"
end

local Reporter = setmetatable({}, { __index = lcov.LcovReporter })
Reporter.__index = Reporter

function Reporter:on_new_file(filename)
  local name = slashes(filename)
  self:write("SF:", names[name] or name, "\n")
end

package.preload["luacov.reporter.dx"] = function()
  return {
    report = function()
      return reporter.report(Reporter)
    end,
  }
end

runner.init({
  statsfile = dir .. "/luacov.stats.out",
  reportfile = dir .. "/luacov.dat",
  reporter = "dx",
  runreport = true,
  include = include,
  exclude = { "/lua/coverage/dx_coverage$" },
})

return names
