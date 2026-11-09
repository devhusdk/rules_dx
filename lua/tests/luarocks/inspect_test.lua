local inspect = require("inspect")

local function check(ok, message)
	if not ok then
		error(message, 2)
	end
end

check(inspect({ 1, 2 }) == "{ 1, 2 }", inspect({ 1, 2 }))
check(inspect("a") == '"a"', inspect("a"))
