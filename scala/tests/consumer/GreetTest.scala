package consumer

object GreetTest {
  def main(args: Array[String]): Unit = {
    if (Greet.hello("dx") != "Hello, dx") sys.exit(1)
  }
}
