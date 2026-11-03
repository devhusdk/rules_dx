local report = require("report")

local function check(ok, message)
	if not ok then
		error(message, 2)
	end
end

local units = {
	{ name = "lamp", qty = 4, price = 1250 },
	{ name = "fuse", qty = 0, price = 35 },
}

local lines = report.lines(units)
check(#lines == 3, "one line per unit plus the total")
check(lines[1] == "lamp 4 x 1250 = 5000", lines[1])
check(lines[2] == "fuse 0 x 35 = 0", lines[2])
check(lines[3] == "total 5000", lines[3])

local dump = report.dump(units)
check(dump:find('name = "lamp"', 1, true), "dump names the unit: " .. dump)
check(dump:find("qty = 4", 1, true), "dump has the quantity: " .. dump)

print("report_test passed")
