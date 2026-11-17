defmodule LockHello do
  @moduledoc false

  def main(_args), do: IO.puts(Jason.decode!(~s("hello lock")))
end
