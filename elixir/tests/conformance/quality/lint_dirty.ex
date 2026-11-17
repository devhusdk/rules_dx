defmodule LintDirty do
  @moduledoc false

  @spec show(term()) :: term()
  def show(a), do: IO.inspect(a)
end
