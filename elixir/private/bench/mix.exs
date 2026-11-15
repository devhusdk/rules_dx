defmodule DxBench.MixProject do
  use Mix.Project

  def project do
    [app: :dx_bench, version: "0.0.0", deps: deps()]
  end

  defp deps do
    [{:benchee, "1.5.1"}]
  end
end
