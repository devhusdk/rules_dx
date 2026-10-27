// Kotlin test whose test_class comes from the target name.
package hello

import org.junit.Assert.assertEquals
import org.junit.Test

class InferredTest {
    @Test
    fun testHello() {
        assertEquals("hello world", Hello.hello("world"))
    }
}
