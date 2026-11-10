# Compiles Elixir sources with docs on the pinned Elixir and writes their public API as versioned JSON.
#
# usage: elixir_dump.exs --srcs FILE --scratch DIR --out FILE
# --srcs lists one source per line as `exec path TAB repo-relative path TAB 0|1 (generated)`.
# The documentation is read from each module's docs chunk with Code.fetch_docs/1, the data
# ExDoc itself renders.

defmodule DxElixirDump do
  @moduledoc false

  def main(argv) do
    opts = parse(argv, %{})
    rows = rows(opts["--srcs"])
    root = File.cwd!()
    scratch = Path.expand(opts["--scratch"])
    File.rm_rf!(scratch)
    File.mkdir_p!(scratch)
    Code.put_compiler_option(:docs, true)
    Code.put_compiler_option(:debug_info, true)
    Code.put_compiler_option(:ignore_module_conflict, true)

    files = Enum.map(rows, fn {exec, _, _} -> Path.expand(exec, root) end)

    case Kernel.ParallelCompiler.compile_to_path(files, scratch, return_diagnostics: true) do
      {:ok, _modules, _warnings} -> :ok
      other -> fail("compilation failed: " <> inspect(other))
    end

    Code.prepend_path(scratch)
    by_path = Map.new(rows, fn {exec, rel, gen} -> {Path.expand(exec, root), {rel, gen == "1"}} end)

    symbols =
      scratch
      |> File.ls!()
      |> Enum.filter(&String.ends_with?(&1, ".beam"))
      |> Enum.sort()
      |> Enum.flat_map(fn beam ->
        module = beam |> String.trim_trailing(".beam") |> String.to_atom()
        symbols(module, by_path)
      end)

    json = JSON.encode!(%{format: 1, language: "elixir", tool: "Code.fetch_docs", version: System.version(), symbols: symbols})
    File.write!(opts["--out"], json)
  end

  defp symbols(module, by_path) do
    {:docs_v1, anno, _, _, moduledoc, _, docs} = fetch(module)
    source = module.module_info(:compile)[:source] |> List.to_string() |> Path.expand()
    {rel, generated} = Map.get(by_path, source) || fail("no listed source for #{inspect(module)}: #{source}")
    name = inspect(module)

    if moduledoc == :hidden do
      []
    else
      mod = entry("module", name, "", 0, "defmodule " <> name, [], text(moduledoc), rel, generated, anno)

      members =
        for {{kind, fun, arity}, fanno, sigs, doc, meta} <- docs,
            doc != :hidden,
            kind in [:function, :macro, :type, :callback, :macrocallback],
            not Map.has_key?(meta, :opaque_hidden),
            do: member(kind, name, fun, arity, fanno, sigs, doc, rel, generated)

      [mod | members]
    end
  end

  defp member(kind, module, fun, arity, anno, sigs, doc, rel, generated) do
    sig = List.first(sigs) || "#{fun}/#{arity}"
    kind_name = if kind == :macrocallback, do: "callback", else: Atom.to_string(kind)
    entry(kind_name, module, Atom.to_string(fun), arity, sig, params(sig), text(doc), rel, generated, anno)
  end

  defp params(sig) do
    case Regex.run(~r/\((.*)\)/s, sig) do
      [_, inner] ->
        inner
        |> String.split(",")
        |> Enum.map(&(&1 |> String.split("\\\\") |> hd() |> String.trim()))
        |> Enum.reject(&(&1 == ""))

      _ ->
        []
    end
  end

  defp entry(kind, module, name, arity, sig, params, doc, rel, generated, anno) do
    %{
      kind: kind, module: module, name: name, arity: arity, signature: sig, params: params,
      doc: doc, file: rel, line: :erl_anno.line(anno), generated: generated
    }
  end

  defp fetch(module) do
    case Code.fetch_docs(module) do
      {:docs_v1, _, _, _, _, _, _} = docs -> docs
      other -> fail("no docs for #{inspect(module)}: #{inspect(other)}")
    end
  end

  defp text(%{"en" => text}), do: text
  defp text(_), do: ""

  defp rows(path) do
    path
    |> File.read!()
    |> String.split("\n", trim: true)
    |> Enum.map(fn line ->
      case String.split(line, "\t") do
        [exec, rel, gen] -> {exec, rel, gen}
        _ -> fail("#{path} has a malformed line: #{inspect(line)}")
      end
    end)
  end

  defp parse([], opts) do
    for flag <- ["--srcs", "--scratch", "--out"], not Map.has_key?(opts, flag), do: fail("#{flag} is required")
    opts
  end

  defp parse([flag, value | rest], opts) when flag in ["--srcs", "--scratch", "--out"] do
    if Map.has_key?(opts, flag), do: fail("#{flag} given more than once")
    parse(rest, Map.put(opts, flag, value))
  end

  defp parse([flag | _], _), do: fail("unknown argument '#{flag}'")

  defp fail(message) do
    IO.puts(:stderr, "elixir_dump: " <> message)
    System.halt(1)
  end
end

DxElixirDump.main(System.argv())
