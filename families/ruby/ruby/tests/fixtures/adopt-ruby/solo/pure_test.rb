# Adopted foreign stdlib-only Ruby test.
require_relative "pure"

got = Pure.hello("world")
if got != "hello world"
  warn "FAIL: got '#{got}'"
  exit 1
end
puts "PASS"
