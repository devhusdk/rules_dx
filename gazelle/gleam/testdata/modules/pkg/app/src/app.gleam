import app/core
import gleam/io
import shared

pub fn main() {
  io.println(core.name(shared.id(1)))
}
