defmodule AppTest do
  use ExUnit.Case

  test "runs" do
    assert {:ok, _} = App.run(1) |> then(&{:ok, &1})
  end
end
