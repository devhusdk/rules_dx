// Foreign Kotlin test: handwritten owner of the test-owned source.
package solo

import org.junit.Assert.assertEquals
import org.junit.Test

class PureTest {
    @Test
    fun testPure() {
        assertEquals("hello world", Pure.pure("world"))
    }
}
