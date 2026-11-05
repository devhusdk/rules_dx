defmodule Stock.LedgerTest do
  use ExUnit.Case

  @moduletag capture_log: true

  alias Stock.Ledger

  setup do
    restart_ledger()
    :ok
  end

  test "the limit comes from app_env" do
    assert Application.get_env(:stock, :max_units) == 60
    assert Ledger.max_units() == 60
  end

  test "the application starts its dependencies" do
    assert :catalog in Application.spec(:stock, :applications)
    assert :jason in Application.spec(:catalog, :applications)
    started = Enum.map(Application.started_applications(), &elem(&1, 0))
    assert Enum.all?([:elixir, :jason, :catalog, :stock], &(&1 in started))
  end

  test "items are counted across adds and takes" do
    assert Ledger.add(item("2 dozen pears")) == {:ok, 24}
    assert Ledger.add(item("3 each pears")) == {:ok, 27}
    assert Ledger.take(item("7 each pears")) == {:ok, 20}
    assert Ledger.count("pears") == 20
    assert Ledger.take(item("21 each pears")) == {:error, {:short, "pears"}}
  end

  test "adds over the configured limit are refused" do
    assert Ledger.add(item("1 crate figs")) == {:ok, 48}
    assert Ledger.add(item("1 dozen figs")) == {:ok, 60}
    assert Ledger.add(item("1 each figs")) == {:error, {:over_limit, "figs", 60}}
    assert Ledger.count("figs") == 60
  end

  test "the snapshot is JSON from catalog" do
    {:ok, _} = Ledger.add(item("4 each plums"))
    {:ok, _} = Ledger.add(item("1 dozen figs"))
    assert Ledger.snapshot() == ~s([{"name":"figs","units":12},{"name":"plums","units":4}])
  end

  test "a killed ledger is restarted empty by its supervisor" do
    {:ok, _} = Ledger.add(item("5 each kiwis"))
    old = Process.whereis(Ledger)
    ref = Process.monitor(old)
    Process.exit(old, :kill)
    assert_receive {:DOWN, ^ref, :process, ^old, :killed}
    new = wait_for_restart(old)
    assert new != old
    assert Ledger.count("kiwis") == 0
    assert [{Ledger, ^new, :worker, [Ledger]}] = Supervisor.which_children(Stock.Supervisor)
  end

  test "a crash inside a call does not take the caller down" do
    {:ok, _} = Ledger.add(item("5 each kiwis"))
    old = Process.whereis(Ledger)
    catch_exit(GenServer.call(Ledger, :unknown))
    assert wait_for_restart(old) != old
    assert Ledger.count("kiwis") == 0
  end

  defp item(text) do
    {:ok, item} = Catalog.parse_item(text)
    item
  end

  defp restart_ledger do
    :ok = Supervisor.terminate_child(Stock.Supervisor, Ledger)
    {:ok, _} = Supervisor.restart_child(Stock.Supervisor, Ledger)
  end

  defp wait_for_restart(old, attempts \\ 100) do
    case Process.whereis(Ledger) do
      pid when is_pid(pid) and pid != old ->
        pid

      _ when attempts > 0 ->
        Process.sleep(10)
        wait_for_restart(old, attempts - 1)
    end
  end
end
