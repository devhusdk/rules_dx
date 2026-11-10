defmodule CatalogTest do
  use ExUnit.Case, async: true

  alias Catalog.Item

  test "units come from the generated table" do
    assert Catalog.Units.all() == [{"each", 1}, {"dozen", 12}, {"crate", 48}]
  end

  test "items are counted in single units" do
    assert Catalog.parse_item("2 dozen pears") == {:ok, %Item{name: "pears", units: 24}}
    assert Catalog.parse_item("1 crate figs") == {:ok, %Item{name: "figs", units: 48}}
  end

  test "malformed items are rejected" do
    assert Catalog.parse_item("dozen pears") == {:error, {:syntax, "dozen pears"}}
    assert Catalog.parse_item("0 each pears") == {:error, {:syntax, "0 each pears"}}
    assert Catalog.parse_item("3 sacks pears") == {:error, {:unknown_unit, "sacks"}}
  end

  test "ledgers encode through jason" do
    assert Catalog.encode(%{"plums" => 4, "figs" => 48}) ==
             ~s([{"name":"figs","units":48},{"name":"plums","units":4}])
  end
end
