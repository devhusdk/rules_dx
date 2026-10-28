# Runs Credo for dx and reports what Credo itself leaves out.
#
# stdout is Credo's own `--format json`. One stderr line, `dx-credo: {...}`,
# lists the requested files Credo dropped because they do not parse.
# Exit codes: 0 no issues and no invalid files, 1 findings, 2 run halted or crashed.

defmodule DxCredo do
  @moduledoc false

  alias Credo.Execution

  @flags_with_value ~w(--config-file --config-name --checks-with-tag --checks-without-tag --format
                       --files-included --files-excluded --only --ignore --min-priority --enable-disabled-checks
                       --working-dir --strategy --mute-exit-status-for --config)

  def main(argv) do
    code =
      try do
        run(argv)
      rescue
        error ->
          IO.puts(:stderr, "dx-credo: crashed: " <> Exception.message(error))
          2
      catch
        kind, reason ->
          IO.puts(:stderr, "dx-credo: crashed: " <> inspect({kind, reason}))
          2
      end

    :erlang.halt(code, flush: true)
  end

  defp run(argv) do
    Credo.Application.start(nil, nil)
    exec = Credo.run(argv ++ ["--format", "json", "--no-color"])
    status = Execution.get_exit_status(exec)
    issues = length(Execution.get_issues(exec))
    invalid = invalid_files(exec, requested(argv))
    IO.puts(:stderr, "dx-credo: " <> Jason.encode!(%{invalid: invalid}))

    cond do
      exec.halted or status >= 128 -> 2
      issues > 0 or invalid != [] or status > 0 -> 1
      true -> 0
    end
  end

  defp requested(argv), do: positional(argv, [])

  defp positional([], acc), do: Enum.reverse(acc)
  defp positional(["--" <> _ = flag, _value | rest], acc) when flag in @flags_with_value, do: positional(rest, acc)
  defp positional(["-" <> _ | rest], acc), do: positional(rest, acc)
  defp positional([file | rest], acc), do: positional(rest, [file | acc])

  defp invalid_files(exec, files) do
    loaded =
      exec
      |> Execution.get_source_files()
      |> MapSet.new(&Path.expand(&1.filename))

    for file <- files, File.regular?(file), not MapSet.member?(loaded, Path.expand(file)), reduce: [] do
      acc ->
        case parse_error(file) do
          nil -> acc
          error -> [error | acc]
        end
    end
    |> Enum.reverse()
  end

  defp parse_error(file) do
    case Code.string_to_quoted(File.read!(file), columns: true, file: file) do
      {:ok, _} ->
        nil

      {:error, {meta, message, token}} ->
        %{
          path: file,
          line: meta_value(meta, :line),
          column: meta_value(meta, :column),
          message: message_text(message, token)
        }
    end
  end

  defp meta_value(meta, key) when is_list(meta), do: Keyword.get(meta, key) || 0
  defp meta_value(line, :line) when is_integer(line), do: line
  defp meta_value(_, _), do: 0

  defp message_text({prefix, suffix}, token), do: to_string(prefix) <> to_string(token) <> to_string(suffix)
  defp message_text(message, token), do: to_string(message) <> to_string(token)
end

DxCredo.main(System.argv())
