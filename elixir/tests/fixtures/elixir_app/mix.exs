defmodule ElixirApp.MixProject do
  use Mix.Project

  def project do
    [app: :elixir_app, version: "0.0.0", deps: deps()]
  end

  defp deps do
    [{:jason, "1.4.4"}]
  end
end
