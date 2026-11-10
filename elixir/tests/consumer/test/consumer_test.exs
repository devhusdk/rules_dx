defmodule ConsumerTest do
  use ExUnit.Case, async: true

  test "greet" do
    assert Consumer.greet("dx") == "hello, dx"
  end
end
