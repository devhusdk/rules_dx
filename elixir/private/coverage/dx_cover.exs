# Loaded by every elixir_test under `bazel coverage`. It cover-compiles the
# workspace apps of the test and writes their line hits as LCOV when the VM exits.
defmodule DxCover do
  def start(apps, sources, output) do
    :cover.start()

    for app <- apps, dir = :code.lib_dir(String.to_atom(app)), is_list(dir) do
      :cover.compile_beam_directory(dir ++ ~c"/ebin")
    end

    System.at_exit(fn _status -> File.write!(output, lcov(sources)) end)
  end

  def lcov(sources) do
    :cover.modules()
    |> Enum.flat_map(&module_lines(&1, sources))
    |> Enum.group_by(&elem(&1, 0), &elem(&1, 1))
    |> Enum.sort()
    |> Enum.map_join(fn {source, lines} -> record(source, lines) end)
  end

  defp module_lines(module, sources) do
    compiled = to_string(module.module_info(:compile)[:source])
    source = Enum.find(sources, &(compiled == &1 or String.ends_with?(compiled, "/" <> &1)))
    {:ok, calls} = :cover.analyse(module, :calls, :line)

    if source do
      for {{_module, line}, count} <- calls, line > 0, do: {source, {line, count}}
    else
      []
    end
  end

  defp record(source, lines) do
    hits = lines |> Enum.group_by(&elem(&1, 0), &elem(&1, 1)) |> Enum.sort()
    da = for {line, counts} <- hits, do: "DA:#{line},#{Enum.sum(counts)}\n"
    covered = Enum.count(hits, fn {_line, counts} -> Enum.sum(counts) > 0 end)
    "SF:#{source}\n#{da}LH:#{covered}\nLF:#{length(hits)}\nend_of_record\n"
  end
end

split = fn name -> name |> System.fetch_env!() |> String.split(",", trim: true) end

DxCover.start(
  split.("DX_ELIXIR_COVER_APPS"),
  split.("DX_ELIXIR_COVER_SOURCES"),
  System.fetch_env!("COVERAGE_OUTPUT_FILE")
)
