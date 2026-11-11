require File.expand_path("greet", File.dirname($0))

if Greet.greet("world") != "goodbye world"
  warn "failing_case"
  exit 1
end
