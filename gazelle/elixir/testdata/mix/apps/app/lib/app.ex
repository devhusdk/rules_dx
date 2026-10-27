defmodule App do
  @moduledoc """
  Mentions Decoy.Prose and Decoy.Other.call() in docs only.
  """
  alias Shared.Util
  require Logger

  def run(value) do
    Logger.info("#{Shared.call()} ran")
    Util.f()
    Jason.encode!(%{value: value, tag: ~S(Decoy.Sigil)})
  end
end
