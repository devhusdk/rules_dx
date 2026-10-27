defmodule App.MixProject do
  use Mix.Project

  @version "0.2.0"

  def project do
    [
      # app: :old_name,
      app: :app,
      version: @version,
      deps: [{:jason, "~> 1.4"}]
    ]
  end

  def application, do: [mod: {App.Application, []}, extra_applications: [:logger]]
end
