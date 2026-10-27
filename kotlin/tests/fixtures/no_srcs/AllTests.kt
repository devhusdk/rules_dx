// Seed Kotlin test run by a kotlin_test with no srcs of its own.
package nosrcs

import org.junit.Assert.assertEquals
import org.junit.Test

class AllTests {
    @Test
    fun testAnswer() {
        assertEquals(42, Impl.answer())
    }
}
