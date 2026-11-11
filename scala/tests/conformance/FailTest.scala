package conformance

object FailTest {
  def main(args: Array[String]): Unit = {
    if (Greet.greet("world") != "goodbye world") {
      throw new AssertionError("failingCase")
    }
  }
}
