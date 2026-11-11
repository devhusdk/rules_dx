defmodule FailTest do
  use ExUnit.Case

  test "failing_case" do
    assert Greet.greet("world") == "goodbye world"
  end
end
