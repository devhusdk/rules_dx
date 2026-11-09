# Seed Ruby test; consumer of ruby_test (plain executable: exit
# code is the verdict; the RSpec runner mapping is qualified separately
# in `ruby/tests/fixtures/rspec/`).
require_relative "hello"

got = Hello.hello("world")
if got != "hello world"
  warn "FAIL: got '#{got}'"
  exit 1
end
puts "PASS"
