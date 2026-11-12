package conformance

import org.junit.Assert

fun main() {
    val greeting = "hello lock"
    Assert.assertTrue(greeting.startsWith("hello"))
    println(greeting)
}
