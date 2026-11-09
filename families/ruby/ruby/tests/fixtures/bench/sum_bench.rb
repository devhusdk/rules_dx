numbers = (1..1000).to_a

DxBench.run(time: 0.2, warmup: 0.1) do |x|
  x.report("sum") { numbers.sum }
  x.report("inject") { numbers.inject(:+) }
end
