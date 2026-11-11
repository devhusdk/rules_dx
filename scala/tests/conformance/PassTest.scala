package conformance

object PassTest {
  def main(args: Array[String]): Unit = {
    if (Greet.greet("world") != "hello world") {
      throw new AssertionError("passingCase")
    }
  }
}
