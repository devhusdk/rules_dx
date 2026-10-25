import app
import gleeunit

pub fn main() {
  gleeunit.main()
}

pub fn name_test() {
  let assert "app" = app.name()
}
