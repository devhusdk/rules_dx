defmodule Hello do
  @moduledoc false

  def main([path]), do: IO.write(File.read!(rlocation(path)))
  def main(_args), do: IO.puts(Greet.greet("world"))

  defp rlocation(path) do
    case System.get_env("RUNFILES_DIR") do
      nil -> manifest_lookup(System.fetch_env!("RUNFILES_MANIFEST_FILE"), path)
      dir -> Path.join(dir, path)
    end
  end

  defp manifest_lookup(manifest, path) do
    key = path <> " "

    manifest
    |> File.read!()
    |> String.split("\n")
    |> Enum.find_value(fn line ->
      if String.starts_with?(line, key) do
        line |> String.replace_prefix(key, "") |> String.trim_trailing("\r")
      end
    end)
  end
end
