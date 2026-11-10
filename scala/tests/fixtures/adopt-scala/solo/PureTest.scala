// Foreign Scala test: handwritten owner of the test-owned source.
package solo

import org.scalatest.flatspec.AnyFlatSpec

class PureTest extends AnyFlatSpec {
  "Pure.pure" should "greet by name" in {
    assert(Pure.pure("world") == "hello world")
  }
}
