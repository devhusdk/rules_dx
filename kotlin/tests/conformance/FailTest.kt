package conformance

fun main() {
    if (Greet.hello("world") != "goodbye world") {
        throw AssertionError("failingCase")
    }
}
