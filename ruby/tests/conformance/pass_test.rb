require File.expand_path("greet", File.dirname($0))

exit 1 unless Greet.greet("world") == "hello world"
