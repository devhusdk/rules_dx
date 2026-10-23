import catalog
import gleam/io
import gleam/list
import stock/ledger
import stockctl/commands

pub fn main() -> Nil {
  case commands.run(ledger.new(commands.default_limit), arguments()) {
    Ok(#(book, lines)) -> {
      list.each(lines, io.println)
      io.println(catalog.to_json(ledger.snapshot(book)))
    }
    Error(message) -> {
      io.println_error("error: " <> message)
      halt(1)
    }
  }
}

@external(erlang, "stockctl_ffi", "arguments")
@external(javascript, "./stockctl_ffi.mjs", "argumentList")
fn arguments() -> List(String)

@external(erlang, "erlang", "halt")
@external(javascript, "./stockctl_ffi.mjs", "halt")
fn halt(code: Int) -> Nil
