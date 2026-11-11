package conformance

fun main() {
    if (Greet.greet("world") != "goodbye world") {
        throw AssertionError("failingCase")
    }
}
