package tests.fixtures.fuzz

object ParseFuzz {
    @JvmStatic
    fun fuzzerTestOneInput(data: ByteArray) {
        if (data.size >= 3 && data[0] == 'f'.code.toByte() && data[1] == 'u'.code.toByte() && data[2] == 'z'.code.toByte()) {
            throw IllegalStateException("planted crash")
        }
    }
}
