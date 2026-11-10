defmodule Geometry do
  @moduledoc """
  Area helpers for simple shapes.
  """

  @typedoc "A width and a height."
  @type size :: {number(), number()}

  @doc """
  Returns the area of a rectangle.
  """
  @spec area(number(), number()) :: number()
  def area(width, height), do: width * height

  @doc "Doubles a number at compile time."
  defmacro double(value), do: quote(do: unquote(value) * 2)

  @doc false
  def hidden, do: :ok
end
