// Foreign Kotlin test: handwritten owner of the test-owned source. The
// generator never emits kotlin_test; `*Test.kt` files stay out of the
// generated library and this rule survives regeneration unchanged.
package greet

import org.junit.Assert.assertEquals
import org.junit.Test

class GreetTest {
    @Test
    fun testGreet() {
        assertEquals("hello world", Greet.greet("world"))
    }

    @Test
    fun testSuffix() {
        assertEquals(" world", Helper.suffix())
    }

    @Test
    fun testGuavaJoin() {
        assertEquals("hello,world", Guava.join())
    }
}
