defmodule Stock.Ledger do
  @moduledoc """
  Supervised ledger of stocked units with a configured per-item limit.
  """

  use GenServer

  @default_max_units 1000

  def start_link(_opts), do: GenServer.start_link(__MODULE__, :ok, name: __MODULE__)

  @doc """
  Returns the per-item limit from the `:stock` application environment.
  """
  def max_units, do: Application.get_env(:stock, :max_units, @default_max_units)

  @doc """
  Adds one parsed item, refusing totals above `max_units/0`.
  """
  def add(%Catalog.Item{} = item), do: GenServer.call(__MODULE__, {:add, item})

  @doc """
  Removes units of one item, refusing to go below zero.
  """
  def take(%Catalog.Item{} = item), do: GenServer.call(__MODULE__, {:take, item})

  @doc """
  Returns the units held for one item name.
  """
  def count(name), do: GenServer.call(__MODULE__, {:count, name})

  @doc """
  Returns the ledger as JSON.
  """
  def snapshot, do: GenServer.call(__MODULE__, :snapshot)

  @impl true
  def init(:ok), do: {:ok, %{}}

  @impl true
  def handle_call({:add, %{name: name, units: units}}, _from, counts) do
    total = Map.get(counts, name, 0) + units

    if total > max_units() do
      {:reply, {:error, {:over_limit, name, max_units()}}, counts}
    else
      {:reply, {:ok, total}, Map.put(counts, name, total)}
    end
  end

  def handle_call({:take, %{name: name, units: units}}, _from, counts) do
    case Map.get(counts, name, 0) - units do
      left when left < 0 -> {:reply, {:error, {:short, name}}, counts}
      0 -> {:reply, {:ok, 0}, Map.delete(counts, name)}
      left -> {:reply, {:ok, left}, Map.put(counts, name, left)}
    end
  end

  def handle_call({:count, name}, _from, counts) do
    {:reply, Map.get(counts, name, 0), counts}
  end

  def handle_call(:snapshot, _from, counts) do
    {:reply, Catalog.encode(counts), counts}
  end
end
