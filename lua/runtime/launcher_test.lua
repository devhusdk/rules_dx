local helper = require("helper")
local runfiles = require("runfiles")

assert(helper.answer == 42, "the module map resolves helper")
assert(arg[1] == "one" and select("#", ...) == 1, "arguments reach the chunk")
assert(runfiles.rlocation("_main/lua/runtime/launcher_test_helper.lua") ~= nil, "runfiles resolves")

