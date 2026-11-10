// Foreign Kotlin stdlib-only unit: adopted without upstream changes.
package solo

object Pure {
    fun pure(name: String): String = "hello " + name
}
