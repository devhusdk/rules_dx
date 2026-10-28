local catalog = require("catalog")
local report = require("report")
local runfiles = require("runfiles")

local USAGE = "usage: units [report | --dump | get NAME]"

local function fail(code, message)
	io.stderr:write(message, "\n")
	os.exit(code)
end

local command = arg[1] or "report"
if command ~= "report" and command ~= "--dump" and command ~= "get" then
	fail(2, USAGE)
end
if command == "get" and (#arg ~= 2) then
	fail(2, USAGE)
end
if command ~= "get" and #arg > 1 then
	fail(2, USAGE)
end

local units, err = catalog.load(runfiles.rlocation(os.getenv("UNITS")))
if not units then
	fail(1, "units: " .. err)
end

if command == "report" then
	print(table.concat(report.lines(units), "\n"))
elseif command == "--dump" then
	print(report.dump(units))
else
	local unit = catalog.get(units, arg[2])
	if not unit then
		fail(4, "units: no unit " .. arg[2])
	end
	print(string.format("%s %d @ %d", unit.name, unit.qty, unit.price))
end
