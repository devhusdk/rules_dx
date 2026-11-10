# RSpec helper; consumer of ruby_library.
require_relative "greeter"

RSpec.configure do |config|
  config.expect_with :rspec do |expectations|
    expectations.syntax = :expect
  end
end
