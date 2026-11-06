local catalog = require("catalog")
local parse = require("catalog.parse")
local runfiles = require("runfiles")

local function check(ok, message)
	if not ok then
		error(message, 2)
	end
end

local unit = parse.row("lamp\t4\t1250")
check(unit and unit.name == "lamp" and unit.qty == 4 and unit.price == 1250, "well formed row")

for _, bad in ipairs({ "lamp", "lamp\tx\t1", "lamp\t1", "\t1\t1", "lamp\t1\t1\t1" }) do
	local row, reason = parse.row(bad)
	check(row == nil and reason:find("malformed row", 1, true), "malformed row is rejected: " .. bad)
end

local units = assert(catalog.load(runfiles.rlocation(os.getenv("UNITS"))))
check(#units == 3, "three units")
check(catalog.get(units, "cable").qty == 12, "get finds a unit")
check(catalog.get(units, "bulb") == nil, "get misses an unknown unit")

local missing, err = catalog.load("/nonexistent/units.tsv")
check(missing == nil and err ~= nil, "a missing file is an error")

local bad_path = os.getenv("TEST_TMPDIR") .. "/bad.tsv"
local bad = assert(io.open(bad_path, "w"))
bad:write("lamp\t4\t1250\n\nbroken\n")
bad:close()
local rejected, reason = catalog.load(bad_path)
check(rejected == nil and reason:find("malformed row", 1, true), "a malformed row fails the load")

print("catalog_test passed")
