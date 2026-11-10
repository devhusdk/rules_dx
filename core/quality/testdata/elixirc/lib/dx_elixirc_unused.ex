defmodule DxElixircUnused do
  def run(value) do
    unused = value + 1
    :ok
  end

  defp helper do
    :ok
  end
end
