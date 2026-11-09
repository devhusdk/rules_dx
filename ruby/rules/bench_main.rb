# frozen_string_literal: true

require "json"

ips_entry = ENV.fetch("DX_BENCH_IPS")
$LOAD_PATH.unshift(File.dirname(File.dirname(File.join(ENV.fetch("TEST_SRCDIR"), ips_entry))))
require "benchmark/ips"

# Runs a benchmark-ips block and writes dx-bench.json for `dx bench`.
module DxBench
  def self.run(time: 0.5, warmup: 0.2, &block)
    report = Benchmark.ips(time, warmup, true, &block)
    results = report.entries.map do |entry|
      ips = entry.stats.central_tendency
      {
        name: entry.label.to_s,
        unit: "s",
        mean: 1.0 / ips,
        stddev: entry.stats.error / (ips * ips),
        iterations: entry.iterations
      }
    end
    File.write(File.join(ENV.fetch("TEST_UNDECLARED_OUTPUTS_DIR"), "dx-bench.json"),
              JSON.generate(schema: "dx-bench/v1", results: results))
  end
end

load File.join(ENV.fetch("TEST_SRCDIR"), ENV.fetch("DX_BENCH_SCRIPT"))
