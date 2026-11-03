defmodule Stockctl do
  @moduledoc """
  Runs stock commands against the ledger.
  """

  @doc """
  Escript entry point.
  """
  def main(args) do
    {:ok, _} = Application.ensure_all_started(:stock)
    {lines, status} = run(Enum.map(args, &List.to_string/1))
    Enum.each(lines, &IO.puts/1)
    System.halt(status)
  end

  @doc """
  Runs commands in order and returns the output lines and exit status.
  """
  def run(commands) do
    Enum.reduce_while(commands, {[], 0}, fn command, {lines, 0} ->
      case command(command) do
        {:ok, line} -> {:cont, {[line | lines], 0}}
        {:error, reason} -> {:halt, {["error: " <> inspect(reason) | lines], 1}}
      end
    end)
    |> then(fn {lines, status} -> {Enum.reverse([Stock.Ledger.snapshot() | lines]), status} end)
  end

  defp command("add " <> text), do: apply_item(text, &Stock.Ledger.add/1)
  defp command("take " <> text), do: apply_item(text, &Stock.Ledger.take/1)
  defp command("count " <> name), do: {:ok, Integer.to_string(Stock.Ledger.count(name))}
  defp command(other), do: {:error, {:unknown_command, other}}

  defp apply_item(text, fun) do
    with {:ok, item} <- Catalog.parse_item(text),
         {:ok, total} <- fun.(item) do
      {:ok, Integer.to_string(total)}
    end
  end
end
