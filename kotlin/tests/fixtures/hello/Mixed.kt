// Kotlin source that calls Java from the same kotlin_library.
package hello

object Mixed {
    fun greet(name: String): String = Greeter.greet(name)
}
