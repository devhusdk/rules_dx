// Seed Kotlin Jupiter test; consumer of the qualified JUnit 6.1.3 runner.
// JUnit 6 adds native Kotlin `suspend` support; the `suspend` shape needs
// kotlinx-coroutines-core plus kotlin-reflect on the classpath and stays a
// documented capability, not fixture-proven here.
package hello

import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Test

class HelloJupiterTest {
    @Test
    fun testHello() {
        assertEquals("hello world", Hello.hello("world"))
    }
}
