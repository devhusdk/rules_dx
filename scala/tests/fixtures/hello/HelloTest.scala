// Seed Scala test; consumer of scala_test (ScalaTest).
package hello

import org.scalatest.flatspec.AnyFlatSpec

class HelloTest extends AnyFlatSpec {
  "Hello.hello" should "greet by name" in {
    assert(Hello.hello("world") == "hello world")
  }
}
