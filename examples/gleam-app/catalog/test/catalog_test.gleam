import catalog.{BadQuantity, Item, MissingName, UnknownUnit}
import catalog/units

pub fn parses_a_unit_test() {
  assert catalog.parse("2 dozen pears") == Ok(Item("pears", 24))
}

pub fn parses_a_bare_count_test() {
  assert catalog.parse("  3   apples ") == Ok(Item("apples", 3))
}

pub fn rejects_bad_input_test() {
  assert catalog.parse("two dozen pears") == Error(BadQuantity("two"))
  assert catalog.parse("0 pears") == Error(BadQuantity("0"))
  assert catalog.parse("2 crates pears") == Error(UnknownUnit("crates"))
  assert catalog.parse("pears") == Error(MissingName)
  assert catalog.describe(UnknownUnit("crates")) == "unknown unit crates"
}

pub fn units_come_from_the_generated_table_test() {
  assert units.all == [#("each", 1), #("pair", 2), #("dozen", 12)]
  assert catalog.unit_size("pair") == Ok(2)
}

pub fn json_round_trips_test() {
  let items = [Item("pears", 24), Item("plums", 2)]
  let text = catalog.to_json(items)
  assert text
    == "[{\"name\":\"pears\",\"units\":24},{\"name\":\"plums\",\"units\":2}]"
  assert catalog.from_json(text) == Ok(items)
}

pub fn json_rejects_the_wrong_shape_test() {
  let assert Error(_) = catalog.from_json("[{\"name\":\"pears\"}]")
  let assert Error(_) = catalog.from_json("[1,")
}
