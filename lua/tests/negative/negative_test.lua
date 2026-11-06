local runfiles = require("runfiles")

local function input(var)
	local rel = assert(os.getenv(var), var .. " is not set")
	return assert(runfiles.rlocation(rel), var .. " is not in the runfiles")
end

local function run(var, ...)
	local command = "'" .. input(var) .. "'"
	for _, arg in ipairs({ ... }) do
		command = command .. " '" .. arg .. "'"
	end
	local pipe = assert(io.popen(command .. " 2>&1"))
	local output = pipe:read("a")
	local _, _, code = pipe:close()
	return output, code
end

local function check(ok, message)
	if not ok then
		error(message, 2)
	end
end

local function has(text, want)
	check(text:find(want, 1, true), "missing '" .. want .. "' in:\n" .. text)
end

local output, code = run("LUAC", "-p", input("SYNTAX_ERROR"))
check(code ~= 0, "luac accepted a syntax error:\n" .. output)
has(output, "syntax_error.lua:4:")

output, code = run("RUNTIME_ERROR")
check(code == 1, output)
has(output, "runtime boom")
has(output, "stack traceback:")
has(output, "runtime_error.lua:2:")

output, code = run("MISSING_DEP")
check(code == 1, output)
has(output, "module 'absent.module' not found")
has(output, "no module 'absent.module' in the deps")

output, code = run("CPATH_DENIED")
check(code == 1, output)
has(output, "module 'socket.core' not found")
check(not output:find(".so'", 1, true), "a C path was searched:\n" .. output)

output, code = run("PROBE", "agent", "3")
check(code == 3, output)
has(output, "hello, agent")
