defmodule HelloTest do
  use ExUnit.Case

  test "greets" do
    assert Hello.greet(" world ") == "hello world"
  end
end
