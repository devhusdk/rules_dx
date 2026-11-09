# Seed Ruby RSpec test; consumer of ruby_test over pinned rspec 3.13.0.
require_relative "spec_helper"

RSpec.describe Greeter do
  it "greets the world" do
    expect(Greeter.greet("world")).to eq("hello world")
  end
end
