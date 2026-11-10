defmodule SumBench do
  use ExUnit.Case

  test "sums a list" do
    list = Enum.to_list(1..1000)
    DxBench.run(%{"reduce" => fn -> Enum.reduce(list, 0, &+/2) end, "sum" => fn -> Enum.sum(list) end}, time: 0.1, warmup: 0.05)
  end
end
