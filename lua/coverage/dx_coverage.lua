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

local root = slashes(os.getenv("TEST_SRCDIR") .. "/" .. os.getenv("TEST_WORKSPACE")) .. "/"

local Reporter = setmetatable({}, { __index = lcov.LcovReporter })
Reporter.__index = Reporter

function Reporter:on_new_file(filename)
  local name = slashes(filename)
  if name:sub(1, #root) == root then
    name = name:sub(#root + 1)
  end
  self:write("SF:", name, "\n")
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
  include = { "^" .. root:gsub("%p", "%%%0") },
  exclude = { "/lua/coverage/dx_coverage$" },
})
