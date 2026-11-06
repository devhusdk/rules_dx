-- LCOV_EXCL_START - reason: runs as a test subprocess, issue: 8545, policy: docs/cli/commands/build-test-coverage.md
local lib = require("lib")

print(lib.greet(arg[1] or "lua"))
local code = tonumber(arg[2] or "0")
os.exit(code)
-- LCOV_EXCL_STOP - reason: end subprocess only, issue: 8545, policy: docs/cli/commands/build-test-coverage.md
