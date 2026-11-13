local inspect = require("inspect")

print(inspect("hello lock") == '"hello lock"' and "hello lock" or "missing")
