defmodule Catalog.Item do
  @moduledoc """
  One stocked item counted in single units.
  """

  @derive Jason.Encoder
  @enforce_keys [:name, :units]
  defstruct [:name, :units]
end
