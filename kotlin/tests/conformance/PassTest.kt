package conformance

fun main() {
    if (Greet.greet("world") != "hello world") {
        throw AssertionError("passingCase")
    }
}
