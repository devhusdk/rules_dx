defmodule DxBench do
  @moduledoc false

  @doc "Runs Benchee on the jobs and writes dx-bench.json to the test outputs directory."
  def run(jobs, opts \\ []) when is_map(jobs) do
    defaults = [time: 1, warmup: 0.2, formatters: [], print: [benchmarking: false, configuration: false, fast_warning: false]]
    suite =
      defaults
      |> Keyword.merge(opts)
      |> Benchee.init()
      |> add_jobs(jobs)
      |> Benchee.collect()
      |> Benchee.statistics()
      |> Benchee.relative_statistics()
    write(suite.scenarios)
    suite
  end

  defp add_jobs(suite, jobs) do
    Enum.reduce(jobs, suite, fn {name, fun}, acc -> Benchee.benchmark(acc, name, fun) end)
  end

  def results(scenarios) do
    for scenario <- scenarios do
      stats = scenario.run_time_data.statistics
      %{name: scenario.name, unit: "ns", mean: stats.average, stddev: stats.std_dev, iterations: stats.sample_size}
    end
  end

  defp write(scenarios) do
    case System.get_env("TEST_UNDECLARED_OUTPUTS_DIR") do
      nil -> :ok
      dir -> File.write!(Path.join(dir, "dx-bench.json"), JSON.encode!(%{schema: "dx-bench/v1", results: results(scenarios)}))
    end
  end
end
