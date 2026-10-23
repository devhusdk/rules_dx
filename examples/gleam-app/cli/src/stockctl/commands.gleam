import catalog.{type Item}
import gleam/int
import gleam/list
import gleam/result
import gleam/string
import stock/ledger.{type Ledger, type LedgerError}

pub const default_limit = 60

pub fn run(
  book: Ledger,
  commands: List(String),
) -> Result(#(Ledger, List(String)), String) {
  use #(book, lines) <- result.map(
    list.try_fold(commands, #(book, []), fn(state, command) {
      let #(book, lines) = state
      use #(book, line) <- result.map(step(book, command))
      #(book, [line, ..lines])
    }),
  )
  #(book, list.reverse(lines))
}

fn step(book: Ledger, command: String) -> Result(#(Ledger, String), String) {
  case string.split_once(command, " ") {
    Ok(#("add", text)) -> change(book, text, ledger.add)
    Ok(#("remove", text)) -> change(book, text, ledger.remove)
    Ok(#("count", name)) -> Ok(#(book, int.to_string(ledger.count(book, name))))
    _ -> Error("unknown command: " <> command)
  }
}

fn change(
  book: Ledger,
  text: String,
  apply: fn(Ledger, Item) -> Result(Ledger, LedgerError),
) -> Result(#(Ledger, String), String) {
  use item <- result.try(
    catalog.parse(text) |> result.map_error(catalog.describe),
  )
  use book <- result.map(apply(book, item) |> result.map_error(ledger.describe))
  #(book, int.to_string(ledger.count(book, item.name)))
}
