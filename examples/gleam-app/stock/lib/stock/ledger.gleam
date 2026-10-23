import catalog.{type Item, Item}
import gleam/dict.{type Dict}
import gleam/int
import gleam/list
import gleam/order
import gleam/string

pub opaque type Ledger {
  Ledger(limit: Int, units: Dict(String, Int))
}

pub type LedgerError {
  OverLimit(name: String, limit: Int)
  NotEnough(name: String, have: Int)
}

pub fn new(limit: Int) -> Ledger {
  Ledger(limit, dict.new())
}

pub fn limit(ledger: Ledger) -> Int {
  ledger.limit
}

pub fn count(ledger: Ledger, name: String) -> Int {
  case dict.get(ledger.units, name) {
    Ok(units) -> units
    Error(Nil) -> 0
  }
}

pub fn add(ledger: Ledger, item: Item) -> Result(Ledger, LedgerError) {
  let total = count(ledger, item.name) + item.units
  case total > ledger.limit {
    True -> Error(OverLimit(item.name, ledger.limit))
    False ->
      Ok(Ledger(..ledger, units: dict.insert(ledger.units, item.name, total)))
  }
}

pub fn remove(ledger: Ledger, item: Item) -> Result(Ledger, LedgerError) {
  let have = count(ledger, item.name)
  case int.compare(item.units, have) {
    order.Gt -> Error(NotEnough(item.name, have))
    order.Eq ->
      Ok(Ledger(..ledger, units: dict.delete(ledger.units, item.name)))
    order.Lt ->
      Ok(
        Ledger(
          ..ledger,
          units: dict.insert(ledger.units, item.name, have - item.units),
        ),
      )
  }
}

pub fn snapshot(ledger: Ledger) -> List(Item) {
  dict.to_list(ledger.units)
  |> list.sort(fn(a, b) { string.compare(a.0, b.0) })
  |> list.map(fn(pair) { Item(pair.0, pair.1) })
}

pub fn describe(error: LedgerError) -> String {
  case error {
    OverLimit(name, limit) ->
      name <> " would exceed the limit of " <> int.to_string(limit)
    NotEnough(name, have) ->
      "only " <> int.to_string(have) <> " " <> name <> " in stock"
  }
}
