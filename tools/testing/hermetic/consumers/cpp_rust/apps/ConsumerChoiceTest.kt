package cli

import org.junit.Assert.assertEquals
import org.junit.Test

class ConsumerChoiceTest {
    @Test
    fun consumerJvmOptionIsKept() {
        assertEquals("kept", System.getProperty("dx.consumer.choice"))
    }
}
