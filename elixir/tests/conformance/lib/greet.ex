defmodule Greet do
  @moduledoc false

  @spec greet(String.t()) :: String.t()
  def greet(name), do: "hello " <> name
end
