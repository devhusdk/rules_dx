defmodule Mixed do
  use Plug.Builder

  def encode(value), do: Jason.encode!(value)
  def doc, do: ExDoc.version()
end
