defmodule PassTest do
  use ExUnit.Case

  test "passing_case" do
    assert Greet.greet("world") == "hello world"
  end
end
