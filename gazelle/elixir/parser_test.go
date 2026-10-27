package elixir

import (
	"slices"
	"strings"
	"testing"
)

func TestParseDefinitions(t *testing.T) {
	src := `defmodule Outer do
  defmodule Inner do
    def f, do: :ok
  end

  defmodule __MODULE__.Child do
  end

  def g do
    fn x -> x end
  end
end

defmodule Sibling.Mod, do: def(h, do: 1)

defprotocol Proto do
  def p(x)
end

defmodule Elixir.Plain do
end
`
	got := ParseDefinitions([]byte(src))
	want := []string{"Outer", "Outer.Child", "Outer.Inner", "Plain", "Proto", "Sibling.Mod"}
	if !slices.Equal(got, want) {
		t.Fatalf("ParseDefinitions = %v, want %v", got, want)
	}
}

func TestParseReferencesDecoys(t *testing.T) {
	src := `defmodule App do
  @moduledoc """
  Uses Decoy.Heredoc and "Decoy.Quoted" in prose.
  """
  @doc ~S"""
  Decoy.Sigil #{Decoy.NoInterp}
  """
  # Decoy.Comment.call()
  def f do
    a = "Decoy.String"
    b = 'Decoy.Charlist'
    c = ~r/Decoy.Regex/
    d = ~S(Decoy.Upper #{Decoy.Upper2})
    e = [Decoy: 1, other: :Decoy]
    g = :"Decoy.Atom"
    h = ?"
    i = "a\"Decoy.Escaped"
    {a, b, c, d, e, g, h, i}
  end
end
`
	if got := ParseReferences([]byte(src)); len(got) != 0 {
		t.Fatalf("ParseReferences = %v, want none", got)
	}
}

func TestParseReferences(t *testing.T) {
	src := `defmodule App.Server do
  use GenServer
  alias Shared.Util
  alias Other.Long, as: Short
  alias Multi.{One, Two.Three}
  alias __MODULE__.State
  require Logger
  import Ecto.Query, only: [from: 2]
  @behaviour App.Behaviour

  defmodule Nested do
    defstruct [:x]
  end

  def run(x) do
    Logger.info("#{Shared.call()} done")
    Util.f()
    Short.g()
    One.h()
    Three.i()
    State.new()
    %Catalog.Item{}
    %Nested{}
    Jason.encode!(x)
    Elixir.Abs.x()
    :erlang.term_to_binary(x)
    &Capture.fun/1
    __MODULE__.helper()
  end
end

defimpl Jason.Encoder, for: App.Server.Nested do
  def encode(v, o), do: Jason.Encode.map(v, o)
end
`
	got := ParseReferences([]byte(src))
	want := []string{
		"Abs", "App.Behaviour", "App.Server.Nested", "App.Server.State", "Capture", "Catalog.Item",
		"Ecto.Query", "GenServer", "Jason", "Jason.Encode", "Jason.Encoder", "Logger",
		"Multi.One", "Multi.Two.Three", "Other.Long", "Shared", "Shared.Util",
	}
	if !slices.Equal(got, want) {
		t.Fatalf("ParseReferences =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}

func TestLexBalancesInterpolation(t *testing.T) {
	src := "x = \"#{ %{a: \"}\"} |> Map.get(:a) }\"\ny = Later.call()\n"
	got := ParseReferences([]byte(src))
	if !slices.Equal(got, []string{"Later", "Map"}) {
		t.Fatalf("ParseReferences = %v", got)
	}
}
