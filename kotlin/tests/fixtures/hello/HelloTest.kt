// Seed Kotlin test; consumer of kotlin_test.
package hello

import org.junit.Assert.assertEquals
import org.junit.Test

class HelloTest {
    @Test
    fun testHello() {
        assertEquals("hello world", Hello.hello("world"))
    }
}
