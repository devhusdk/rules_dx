// scala_test relying on rules_scala's implicit scalatest dependency (no explicit deps).
package hello

import org.scalatest.flatspec.AnyFlatSpec

class ImplicitTest extends AnyFlatSpec {
  "Hello.hello" should "greet by name" in {
    assert(Hello.hello("implicit") == "hello implicit")
  }
}
