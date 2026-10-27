defmodule Legacy.BTest do
  use ExUnit.Case

  test "level" do
    assert Legacy.level() == 1
  end
end
