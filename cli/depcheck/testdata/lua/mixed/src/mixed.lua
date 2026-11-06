local socket = require("socket.http")
local cjson = require "cjson"
local lpeg = require("lpeg")
return { socket = socket, cjson = cjson, lpeg = lpeg }
