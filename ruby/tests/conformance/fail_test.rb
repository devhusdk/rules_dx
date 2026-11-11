require_relative "greet"

if Greet.greet("world") != "goodbye world"
  warn "failing_case"
  exit 1
end
