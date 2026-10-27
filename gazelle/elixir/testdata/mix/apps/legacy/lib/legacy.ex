defmodule Legacy do
  @moduledoc false
  def level, do: Application.fetch_env!(:legacy, :level)
end
