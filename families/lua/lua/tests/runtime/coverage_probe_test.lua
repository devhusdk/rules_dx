local lib = require("lib")

local dir = os.getenv("COVERAGE_DIR")
if dir then
  local runner = require("luacov.runner")
  assert(runner.initialized, "luacov runs under coverage")
  assert(package.loaded["luacov.reporter.lcov"], "the lcov reporter is loaded")
  local covered = {}
  for _, name in pairs(require("dx_coverage")) do
    covered[name] = true
  end
  assert(covered["lua/tests/runtime/lib.lua"], "lib.lua is named by its runfiles key")
else
  assert(package.loaded["luacov.runner"] == nil, "luacov stays out of plain tests")
end

assert(lib.greet("x") == "hello, x", "lib.greet")
