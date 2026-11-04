package diagnostics

object UncheckedCast {
    fun names(value: Any): List<String> {
        return value as List<String>
    }
}
