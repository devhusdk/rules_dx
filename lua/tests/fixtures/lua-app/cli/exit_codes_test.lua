local runfiles = require("runfiles")

local bin = runfiles.rlocation(arg[1])
assert(bin, "units binary is in the runfiles")

local failures = 0

local function check(want_code, want_out, want_err, ...)
	local args = { ... }
	local errfile = os.tmpname()
	local command = "\"" .. bin .. "\" " .. table.concat(args, " ") .. " 2>\"" .. errfile .. "\""
	if package.config:sub(1, 1) == "\\" then
		-- cmd /c drops the outer quotes of a command that starts and ends with one.
		command = '"' .. command .. '"'
	end
	local pipe = assert(io.popen(command, "r"))
	local out = pipe:read("a"):gsub("[\r\n]+$", "")
	local _, _, code = pipe:close()
	local file = assert(io.open(errfile, "r"))
	local err = file:read("a"):gsub("[\r\n]+$", "")
	file:close()
	os.remove(errfile)
	if code ~= want_code or out ~= want_out or err ~= want_err then
		print(string.format("FAIL: %s -> %s '%s' '%s' (want %d '%s' '%s')", table.concat(args, " "), tostring(code), out, err, want_code, want_out, want_err))
		failures = failures + 1
	end
end

local usage = "usage: units [report | --dump | get NAME]"
local report = "lamp 4 x 1250 = 5000\ncable 12 x 199 = 2388\nfuse 0 x 35 = 0\ntotal 7388"

check(0, report, "")
check(0, report, "", "report")
check(0, "cable 12 @ 199", "", "get", "cable")
check(4, "", "units: no unit bulb", "get", "bulb")
check(2, "", usage, "sell")
check(2, "", usage, "get")
check(2, "", usage, "report", "extra")

os.exit(failures == 0 and 0 or 1)
