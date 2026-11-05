defmodule HexDemo.MixProject do
  use Mix.Project

  def project do
    [
      app: :hex_demo,
      version: "0.0.0",
      description: "Fixture package for hex_deploy.",
      package: [licenses: ["Apache-2.0"], links: %{}]
    ]
  end
end
