// Foreign Scala test: handwritten owner of the test-owned source. The
// generator never emits scala_test; `*Test.scala` files stay out of the
// generated library and this rule survives regeneration unchanged.
package greet

import org.scalatest.flatspec.AnyFlatSpec

class GreetTest extends AnyFlatSpec {
  "Greet.greet" should "greet by name" in {
    assert(Greet.greet("world") == "hello world")
  }

  "Helper.suffix" should "return the suffix" in {
    assert(Helper.suffix() == " world")
  }

  "Guava.join" should "join via the pinned guava dep" in {
    assert(Guava.join() == "hello,world")
  }
}
