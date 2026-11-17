local M = {}

--- Parses one tab separated row into a unit, or returns nil and a reason.
function M.row(line)
	local name, qty, price = line:match("^([^\t]+)\t(%d+)\t(%d+)$")
	if not name then
		return nil, "malformed row: " .. line
	end
	return { name = name, qty = tonumber(qty), price = tonumber(price) }
end

return M
