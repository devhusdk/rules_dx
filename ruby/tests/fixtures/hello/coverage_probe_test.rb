# Cross-host SF probe for Ruby coverage (#3954). Under `bazel coverage` a child
# Ruby run with the inherited RUBYOPT and RUBYLIB must write LCOV that names
# hello.rb with at least one hit; under `bazel test` SimpleCov stays unloaded.
require "rbconfig"

HELLO_SF = "ruby/tests/fixtures/hello/hello.rb".freeze

# LCOV_EXCL_START - reason: failure path of the probe, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
def probe_fail(message)
  warn "FAIL: #{message}"
  exit 1
end
# LCOV_EXCL_STOP - reason: end failure path, issue: 3954, policy: docs/cli/commands/build-test-coverage.md

def hello_record(lcov)
  lcov.split("end_of_record").find do |record|
    record.lines.any? { |line| line.chomp.sub(%r{\ASF:(\./)?}, "SF:") == "SF:#{HELLO_SF}" }
  end
end

def probe_coverage
  out = File.join(ENV.fetch("TEST_TMPDIR"), "child.dat")
  child = File.join(File.dirname(__FILE__), "hello_test.rb")
  ok = system({ "COVERAGE_OUTPUT_FILE" => out }, RbConfig.ruby, child)
  probe_fail("child '#{child}' exited #{$?.inspect}") unless ok # LCOV_EXCL_LINE - reason: failure path, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
  lcov = File.exist?(out) ? File.read(out) : ""
  record = hello_record(lcov)
  probe_fail("no SF:#{HELLO_SF} in child LCOV:\n#{lcov}") if record.nil? # LCOV_EXCL_LINE - reason: failure path, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
  sf = record.lines.find { |line| line.start_with?("SF:") }.chomp
  hit = record.lines.any? { |line| line.start_with?("DA:") && line.split(",")[1].to_i.positive? }
  probe_fail("#{sf} has no hit DA line:\n#{record}") unless hit # LCOV_EXCL_LINE - reason: failure path, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
  puts "PASS: #{sf}"
end

if ENV["COVERAGE"] == "1"
  probe_coverage
else
  # LCOV_EXCL_START - reason: runs only outside coverage, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
  probe_fail("SimpleCov loaded outside coverage") if defined?(SimpleCov)
  puts "PASS: no coverage"
  # LCOV_EXCL_STOP - reason: end outside-coverage branch, issue: 3954, policy: docs/cli/commands/build-test-coverage.md
end
