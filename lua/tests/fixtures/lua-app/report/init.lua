local inspect = require("inspect")

local M = {}

--- Returns the report lines and the total value for units.
function M.lines(units)
	local lines = {}
	local total = 0
	for _, unit in ipairs(units) do
		local value = unit.qty * unit.price
		lines[#lines + 1] = string.format("%s %d x %d = %d", unit.name, unit.qty, unit.price, value)
		total = total + value
	end
	lines[#lines + 1] = "total " .. total
	return lines
end

--- Returns a readable dump of units, with keys in a stable order.
function M.dump(units)
	return inspect(units)
end

return M
