local runfiles = require("runfiles")

local file = assert(io.open(runfiles.rlocation(arg[1]), "rb"))
io.write(file:read("a"))
file:close()
