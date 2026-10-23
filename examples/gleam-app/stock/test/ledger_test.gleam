import catalog.{Item}
import stock/ledger.{NotEnough, OverLimit}

pub fn adds_up_to_the_limit_test() {
  let assert Ok(book) = ledger.add(ledger.new(30), Item("pears", 24))
  let assert Ok(book) = ledger.add(book, Item("pears", 6))
  assert ledger.count(book, "pears") == 30
  assert ledger.add(book, Item("pears", 1)) == Error(OverLimit("pears", 30))
  assert ledger.limit(book) == 30
}

pub fn removes_and_forgets_empty_items_test() {
  let assert Ok(book) = ledger.add(ledger.new(60), Item("plums", 5))
  let assert Ok(book) = ledger.add(book, Item("apples", 2))
  let assert Ok(book) = ledger.remove(book, Item("plums", 2))
  assert ledger.snapshot(book) == [Item("apples", 2), Item("plums", 3)]
  let assert Ok(book) = ledger.remove(book, Item("apples", 2))
  assert ledger.snapshot(book) == [Item("plums", 3)]
  assert ledger.count(book, "apples") == 0
}

pub fn refuses_to_remove_more_than_it_has_test() {
  let book = ledger.new(60)
  let assert Error(error) = ledger.remove(book, Item("figs", 1))
  assert error == NotEnough("figs", 0)
  assert ledger.describe(error) == "only 0 figs in stock"
  assert ledger.describe(OverLimit("figs", 60))
    == "figs would exceed the limit of 60"
}
