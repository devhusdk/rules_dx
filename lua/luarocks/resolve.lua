local rockspec = require("rockspec")

local path = arg[1]
if not path then
  io.stderr:write("usage: resolve <file.rockspec>\n")
  os.exit(2)
end
local file, open_problem = io.open(path, "rb")
if not file then
  io.stderr:write(open_problem .. "\n")
  os.exit(1)
end
local text = file:read("a")
file:close()

local resolved, problem = rockspec.resolve(text, path:match("([^/\\]+)$"))
if not resolved then
  io.stderr:write(problem .. "\n")
  os.exit(1)
end
io.write(rockspec.encode(resolved), "\n")
