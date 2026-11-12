defmodule Catalog do
  @moduledoc """
  Parses stock commands and encodes ledgers as JSON.
  """

  alias Catalog.Item

  @doc """
  Parses `"<count> <unit> <name>"` into an item measured in single units.
  """
  def parse_item(text) do
    with [count, unit, name] <- String.split(text, " ", trim: true),
         {count, ""} when count > 0 <- Integer.parse(count),
         {:ok, size} <- unit_size(unit) do
      {:ok, %Item{name: name, units: count * size}}
    else
      {:error, _} = error -> error
      _ -> {:error, {:syntax, text}}
    end
  end

  @doc """
  Returns the number of single units in one named unit.
  """
  def unit_size(unit) do
    case List.keyfind(Catalog.Units.all(), unit, 0) do
      {^unit, size} -> {:ok, size}
      nil -> {:error, {:unknown_unit, unit}}
    end
  end

  @doc """
  Encodes a name-to-units map as JSON with sorted keys.
  """
  def encode(counts) do
    counts
    |> Enum.sort()
    |> Enum.map(fn {name, units} -> %Item{name: name, units: units} end)
    |> Jason.encode!()
  end
end
