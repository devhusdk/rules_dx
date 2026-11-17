# Adopted foreign RSpec spec (bundle-backed).
require_relative "greet"

RSpec.describe Greet do
  it "greets the world" do
    expect(Greet.hello("world")).to eq("hello world")
  end
end
