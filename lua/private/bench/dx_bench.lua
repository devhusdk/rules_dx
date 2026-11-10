local M = {}

local function now()
  return os.clock()
end

local function measure(fn, iterations, rounds)
  local times = {}
  for r = 1, rounds do
    local start = now()
    for _ = 1, iterations do
      fn()
    end
    times[r] = (now() - start) / iterations
  end
  local mean = 0
  for _, t in ipairs(times) do
    mean = mean + t
  end
  mean = mean / rounds
  local var = 0
  for _, t in ipairs(times) do
    var = var + (t - mean) ^ 2
  end
  local stddev = rounds > 1 and math.sqrt(var / (rounds - 1)) or 0
  return mean, stddev
end

local function quote(s)
  return '"' .. s:gsub('[%c"\\]', function(c)
    return string.format("\\u%04x", c:byte())
  end) .. '"'
end

function M.run(cases, opts)
  opts = opts or {}
  local iterations = opts.iterations or 100
  local rounds = opts.rounds or 5
  local rows = {}
  for _, case in ipairs(cases) do
    local mean, stddev = measure(case[2], iterations, rounds)
    rows[#rows + 1] = string.format(
      '{"name":%s,"unit":"s","mean":%.12g,"stddev":%.12g,"iterations":%d}',
      quote(case[1]), mean, stddev, iterations * rounds)
  end
  local dir = os.getenv("TEST_UNDECLARED_OUTPUTS_DIR")
  assert(dir, "TEST_UNDECLARED_OUTPUTS_DIR is not set")
  local file = assert(io.open(dir .. "/dx-bench.json", "w"))
  file:write('{"schema":"dx-bench/v1","results":[' .. table.concat(rows, ",") .. "]}")
  file:close()
  return #rows
end

return M
