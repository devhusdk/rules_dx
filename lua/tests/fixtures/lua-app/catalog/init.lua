local parse = require("catalog.parse")

local M = {}

--- Reads every unit from the file at path. Blank lines are skipped.
function M.load(path)
	local file, err = io.open(path, "r")
	if not file then
		return nil, err
	end
	local units = {}
	for line in file:lines() do
		if line ~= "" then
			local unit, reason = parse.row(line)
			if not unit then
				file:close()
				return nil, reason
			end
			units[#units + 1] = unit
		end
	end
	file:close()
	return units
end

--- Returns the unit called name, or nil.
function M.get(units, name)
	for _, unit in ipairs(units) do
		if unit.name == name then
			return unit
		end
	end
	return nil
end

return M
