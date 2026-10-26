local lib = require("lib")

print(lib.greet(arg[1] or "lua"))
local code = tonumber(arg[2] or "0")
os.exit(code)
