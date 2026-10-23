defmodule StockctlTest do
  use ExUnit.Case

  setup do
    :ok = Supervisor.terminate_child(Stock.Supervisor, Stock.Ledger)
    {:ok, _} = Supervisor.restart_child(Stock.Supervisor, Stock.Ledger)
    :ok
  end

  test "commands run in process" do
    assert Stockctl.run(["add 2 dozen pears", "take 4 each pears", "count pears"]) ==
             {["24", "20", "20", ~s([{"name":"pears","units":20}])], 0}
  end

  test "the first failing command stops the run" do
    assert Stockctl.run(["add 1 crate figs", "add 2 dozen figs", "count figs"]) ==
             {["48", ~s(error: {:over_limit, "figs", 60}), ~s([{"name":"figs","units":48}])], 1}
  end

  test "the escript runs on the pinned runtime" do
    assert stockctl(["add 3 each plums", "count plums", "count figs"]) ==
             {"3\n3\n0\n[{\"name\":\"plums\",\"units\":3}]\n", 0}
  end

  test "the escript rejects malformed input" do
    assert stockctl(["add plums"]) == {"error: {:syntax, \"plums\"}\n[]\n", 1}
    assert stockctl(["sell 3 each plums"]) == {"error: {:unknown_command, \"sell 3 each plums\"}\n[]\n", 1}
  end

  defp stockctl(args) do
    root = Path.join(System.fetch_env!("TEST_SRCDIR"), System.fetch_env!("TEST_WORKSPACE"))
    System.cmd(Path.join(root, "examples/elixir-app/cli/stockctl"), args, stderr_to_stdout: true)
  end
end
