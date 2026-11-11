defmodule Hello do
  @moduledoc false

  def main(_args), do: IO.puts(Greet.greet("world"))
end
