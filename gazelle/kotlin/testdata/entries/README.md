# Entry naming and package-level generation

Verifies a `Main.kt` source without a `main` function stays an ordinary
library source (thin `kotlin_binary` entries are never inferred; `main`
stays handwritten per `gazelle/kotlin/lang.go`), and `HelperTest.kt` stays
out of the library and gets its own `kotlin_test(helper_test)` that depends on
the library.
