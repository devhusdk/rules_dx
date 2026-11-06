defmodule Mixed.MixProject do
  use Mix.Project

  def project do
    [app: :mixed, version: "0.1.0", deps: deps()]
  end

  defp deps do
    [
      {:jason, "~> 1.4"},
      {:telemetry, "~> 1.2"},
      {:ex_doc, "~> 0.31", only: :dev, runtime: false}
    ]
  end
end
