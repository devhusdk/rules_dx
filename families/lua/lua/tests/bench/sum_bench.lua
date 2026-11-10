local bench = require("dx_bench")

local numbers = {}
for i = 1, 1000 do
  numbers[i] = i
end

bench.run({
  { "sum_loop", function()
    local total = 0
    for _, n in ipairs(numbers) do
      total = total + n
    end
    return total
  end },
  { "sum_concat", function()
    return #table.concat(numbers, "+")
  end },
}, { iterations = 20, rounds = 3 })
