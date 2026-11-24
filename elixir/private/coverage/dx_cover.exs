# Loaded by every elixir_test under `bazel coverage`. It cover-compiles the
# workspace apps of the test and writes their line hits as LCOV when the VM exits.
defmodule DxCover do
  def start(apps, sources, output) do
    :cover.start()

    ebins =
      for app <- apps,
          dir = :code.lib_dir(String.to_atom(app)),
          is_list(dir),
          do: dir ++ ~c"/ebin"

    files = Map.new(Enum.flat_map(ebins, &beam_files/1))
    Enum.each(ebins, &:cover.compile_beam_directory/1)
    System.at_exit(fn _status -> File.write!(output, lcov(files, sources)) end)
  end

  defp beam_files(ebin) do
    for beam <- Path.wildcard(Path.join(to_string(ebin), "*.beam")) do
      {:ok, {module, [debug_info: info]}} =
        :beam_lib.chunks(String.to_charlist(beam), [:debug_info])

      {module, source_file(info)}
    end
  end

  defp source_file({:debug_info_v1, _backend, {:elixir_v1, %{file: file}, _specs}}),
    do: to_string(file)

  defp source_file(_info), do: ""

  def lcov(files, sources) do
    :cover.modules()
    |> Enum.flat_map(&module_lines(&1, Map.get(files, &1, ""), sources))
    |> Enum.group_by(&elem(&1, 0), &elem(&1, 1))
    |> Enum.sort()
    |> Enum.map_join(fn {source, lines} -> record(source, lines) end)
  end

  defp module_lines(module, compiled, sources) do
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
