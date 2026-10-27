local lib = require("lib")

local M = {}

function M.shout(name)
  return string.upper(lib.greet(name))
end

return M
