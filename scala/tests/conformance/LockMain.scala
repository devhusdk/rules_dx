package conformance

import org.junit.Assert

object LockMain {
  def main(args: Array[String]): Unit = {
    val greeting = "hello lock"
    Assert.assertTrue(greeting.startsWith("hello"))
    println(greeting)
  }
}
