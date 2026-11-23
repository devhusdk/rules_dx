defmodule UnusedVariable do
  def add(a, b) do
    unused = a
    b
  end
end
