package app

import org.scalatest.flatspec.AnyFlatSpec

class PureTest extends AnyFlatSpec {
  "Pure.pure" should "greet by name" in {
    assert(Pure.pure("world") == "hello world")
  }
}
