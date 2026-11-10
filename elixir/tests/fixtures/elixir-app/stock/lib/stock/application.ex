defmodule Stock.Application do
  @moduledoc false

  use Application

  @impl true
  def start(_type, _args) do
    Supervisor.start_link([Stock.Ledger], strategy: :one_for_one, name: Stock.Supervisor)
  end
end
