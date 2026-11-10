local bench = require("dx_bench")

assert(bench.run({ { "noop", function() end } }, { iterations = 2, rounds = 2 }) == 1)
local file = assert(io.open(os.getenv("TEST_UNDECLARED_OUTPUTS_DIR") .. "/dx-bench.json"))
local text = file:read("a")
file:close()
assert(text:find('"schema":"dx-bench/v1"', 1, true), text)
assert(text:find('"name":"noop"', 1, true), text)
