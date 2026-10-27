defmodule App.Application do
  @moduledoc false
  use Application

  def start(_type, _args) do
    Supervisor.start_link([App.Worker], strategy: :one_for_one)
  end
end
