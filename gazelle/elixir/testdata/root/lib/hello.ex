defmodule Hello do
  @moduledoc false
  def greet(name), do: "hello #{String.trim(name)}"
end
