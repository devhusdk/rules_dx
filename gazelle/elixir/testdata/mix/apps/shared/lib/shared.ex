defmodule Shared do
  @moduledoc "Shared helpers."

  def call, do: "shared"

  defmodule Util do
    @moduledoc false
    def f, do: :ok
  end
end
