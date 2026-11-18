package conformance

fun main() {
    if (Greet.hello("world") != "hello world") {
        throw AssertionError("passingCase")
    }
}
