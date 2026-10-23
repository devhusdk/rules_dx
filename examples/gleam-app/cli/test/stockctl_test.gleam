import catalog.{Item}
import stock/ledger
import stockctl/commands

pub fn runs_commands_in_order_test() {
  let assert Ok(#(book, lines)) =
    commands.run(ledger.new(commands.default_limit), [
      "add 2 dozen pears",
      "remove 1 pair pears",
      "add 3 plums",
      "count pears",
    ])
  assert lines == ["24", "22", "3", "22"]
  assert ledger.snapshot(book) == [Item("pears", 22), Item("plums", 3)]
}

pub fn stops_at_the_first_failure_test() {
  let book = ledger.new(commands.default_limit)
  assert commands.run(book, ["add 6 dozen pears"])
    == Error("pears would exceed the limit of 60")
  assert commands.run(book, ["add 2 crates pears", "count pears"])
    == Error("unknown unit crates")
  assert commands.run(book, ["sell pears"])
    == Error("unknown command: sell pears")
}
