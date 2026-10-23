import catalog/units
import gleam/dynamic/decode
import gleam/int
import gleam/json
import gleam/list
import gleam/result
import gleam/string

pub type Item {
  Item(name: String, units: Int)
}

pub type ParseError {
  BadQuantity(word: String)
  UnknownUnit(unit: String)
  MissingName
}

pub fn parse(text: String) -> Result(Item, ParseError) {
  case words(text) {
    [quantity, unit, name] -> {
      use count <- result.try(quantity_of(quantity))
      use size <- result.try(unit_size(unit))
      Ok(Item(name, count * size))
    }
    [quantity, name] -> {
      use count <- result.try(quantity_of(quantity))
      Ok(Item(name, count))
    }
    _ -> Error(MissingName)
  }
}

pub fn unit_size(unit: String) -> Result(Int, ParseError) {
  list.key_find(units.all, unit)
  |> result.replace_error(UnknownUnit(unit))
}

pub fn describe(error: ParseError) -> String {
  case error {
    BadQuantity(word) -> "bad quantity " <> word
    UnknownUnit(unit) -> "unknown unit " <> unit
    MissingName -> "want QUANTITY [UNIT] NAME"
  }
}

pub fn to_json(items: List(Item)) -> String {
  json.array(items, fn(item) {
    json.object([
      #("name", json.string(item.name)),
      #("units", json.int(item.units)),
    ])
  })
  |> json.to_string
}

pub fn from_json(text: String) -> Result(List(Item), json.DecodeError) {
  let item = {
    use name <- decode.field("name", decode.string)
    use units <- decode.field("units", decode.int)
    decode.success(Item(name, units))
  }
  json.parse(text, decode.list(item))
}

fn words(text: String) -> List(String) {
  string.split(text, " ")
  |> list.filter(fn(word) { word != "" })
}

fn quantity_of(word: String) -> Result(Int, ParseError) {
  case int.parse(word) {
    Ok(count) if count > 0 -> Ok(count)
    _ -> Error(BadQuantity(word))
  }
}
